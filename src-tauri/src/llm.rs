use crate::error::{AppError, AppResult};
use crate::models::{EngineKind, LlmConfig, ModelInfo};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a str>,
    /// OpenAI/Ollama 兼容字段：推理模型传 "none" 可关闭思考。
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    #[serde(default)]
    content: Option<String>,
    /// 部分推理模型把最终答案放在 reasoning_content / reasoning 字段。
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    owned_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelEntry {
    name: String,
}

fn build_client(config: &LlmConfig) -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs.max(10)))
        .build()
        .map_err(|e| AppError::Llm(format!("HTTP 客户端构建失败: {e}")))
}

fn endpoint(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn apply_auth(req: reqwest::RequestBuilder, config: &LlmConfig) -> reqwest::RequestBuilder {
    match &config.api_key {
        Some(key) if !key.trim().is_empty() => req.bearer_auth(key.trim()),
        _ => req,
    }
}

/// 探测可用模型。优先使用 OpenAI 兼容的 `/models`；
/// 若为 Ollama 且失败，则回退到原生 `/api/tags`。
pub async fn list_models(config: &LlmConfig) -> AppResult<Vec<ModelInfo>> {
    let client = build_client(config)?;
    let url = endpoint(&config.base_url, "models");
    let resp = apply_auth(client.get(&url), config).send().await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let parsed: ModelsResponse = r
                .json()
                .await
                .map_err(|e| AppError::Llm(format!("解析模型列表失败: {e}")))?;
            let mut models: Vec<ModelInfo> = parsed
                .data
                .into_iter()
                .map(|m| ModelInfo {
                    id: m.id,
                    owned_by: m.owned_by,
                })
                .collect();
            models.sort_by(|a, b| a.id.cmp(&b.id));
            Ok(models)
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            // Ollama 原生回退
            if matches!(config.engine, EngineKind::Ollama) {
                if let Ok(models) = ollama_list_models(&client, config).await {
                    return Ok(models);
                }
            }
            Err(AppError::Llm(format!(
                "模型列表请求失败 ({status}): {}",
                truncate(&body, 300)
            )))
        }
        Err(e) => {
            if matches!(config.engine, EngineKind::Ollama) {
                if let Ok(models) = ollama_list_models(&client, config).await {
                    return Ok(models);
                }
            }
            Err(AppError::Llm(format!(
                "无法连接引擎 {url}: {e}。请确认本地服务已启动。"
            )))
        }
    }
}

async fn ollama_list_models(
    client: &reqwest::Client,
    config: &LlmConfig,
) -> AppResult<Vec<ModelInfo>> {
    let root = config
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1");
    let url = endpoint(root, "api/tags");
    let parsed: OllamaTagsResponse = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Llm(format!("Ollama tags 请求失败: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Llm(format!("解析 Ollama 模型列表失败: {e}")))?;
    let mut models: Vec<ModelInfo> = parsed
        .models
        .into_iter()
        .map(|m| ModelInfo {
            id: m.name,
            owned_by: Some("ollama".into()),
        })
        .collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

/// 调用 OpenAI 兼容的 chat/completions，强制 JSON 输出。
/// 默认关闭推理模型的思考模式；若引擎不认识该字段则自动降级重试。
pub async fn chat_json(
    config: &LlmConfig,
    system: &str,
    user: &str,
    json_mode: bool,
) -> AppResult<String> {
    chat_completion(config, system, user, json_mode, true).await
}

async fn chat_completion(
    config: &LlmConfig,
    system: &str,
    user: &str,
    json_mode: bool,
    disable_thinking: bool,
) -> AppResult<String> {
    let client = build_client(config)?;
    let url = endpoint(&config.base_url, "chat/completions");

    let request = ChatRequest {
        model: &config.model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system,
            },
            ChatMessage {
                role: "user",
                content: user,
            },
        ],
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        stream: false,
        response_format: if json_mode {
            Some(json!({ "type": "json_object" }))
        } else {
            None
        },
        format: None,
        reasoning_effort: disable_thinking.then_some("none"),
    };

    let resp = apply_auth(client.post(&url), config)
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::Llm(format!("请求 {url} 失败: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // 某些引擎不认识 reasoning_effort；去掉思考控制后重试一次。
        if disable_thinking && matches!(status.as_u16(), 400 | 404 | 422) {
            return Box::pin(chat_completion(config, system, user, json_mode, false)).await;
        }
        // 某些引擎不支持 response_format，降级重试一次。
        if json_mode {
            return Box::pin(chat_completion(
                config,
                system,
                user,
                false,
                disable_thinking,
            ))
            .await;
        }
        return Err(AppError::Llm(format!(
            "LLM 返回错误 ({status}): {}",
            truncate(&body, 400)
        )));
    }

    let parsed: ChatResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Llm(format!("解析 LLM 响应失败: {e}")))?;
    let message = parsed.choices.into_iter().next().map(|c| c.message);
    let content = message.map(|m| {
        let primary = m.content.unwrap_or_default();
        if primary.trim().is_empty() {
            m.reasoning_content.or(m.reasoning).unwrap_or_default()
        } else {
            primary
        }
    });
    content
        .filter(|c| !c.trim().is_empty())
        .ok_or_else(|| AppError::Llm("LLM 响应为空".into()))
}

/// 简单连通性探测。
pub async fn health_check(config: &LlmConfig) -> AppResult<bool> {
    let models = list_models(config).await?;
    Ok(!models.is_empty() || config.model.is_empty())
}

fn ollama_root(base: &str) -> String {
    base.trim_end_matches('/')
        .trim_end_matches("/v1")
        .to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuStatus {
    pub gpus: Vec<GpuInfo>,
    /// 当前模型占用显存（仅 Ollama `/api/ps` 提供）。
    pub model_vram_bytes: Option<u64>,
    /// 建议并发数（基于显存余量启发式）。
    pub suggested_concurrency: u32,
}

/// 通过 `nvidia-smi` 探测 NVIDIA 显卡显存（尽力而为，非 NVIDIA 返回空）。
fn detect_gpus() -> Vec<GpuInfo> {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<String> = line.split(',').map(|s| s.trim().to_string()).collect();
                if parts.len() < 3 {
                    return None;
                }
                let total_mib: u64 = parts[1].parse().ok()?;
                let used_mib: u64 = parts[2].parse().ok()?;
                Some(GpuInfo {
                    name: parts[0].clone(),
                    total_bytes: total_mib * 1024 * 1024,
                    used_bytes: used_mib * 1024 * 1024,
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// 汇总 GPU 显存与当前模型占用，并给出建议并发数。
pub async fn gpu_status(config: &LlmConfig) -> GpuStatus {
    let gpus = detect_gpus();
    let model_vram_bytes = match config.engine {
        EngineKind::Ollama => ollama_ps_vram(config).await,
        _ => None,
    };

    // 启发式：模型显存占用低于单卡总显存的 80% 时才建议并发。
    let mut suggested_concurrency = 1u32;
    if let Some(model_vram) = model_vram_bytes {
        if let Some(gpu) = gpus.iter().max_by_key(|g| g.total_bytes) {
            if gpu.total_bytes > 0 {
                let ratio = model_vram as f64 / gpu.total_bytes as f64;
                suggested_concurrency = if ratio <= 0.5 {
                    4
                } else if ratio <= 0.65 {
                    3
                } else if ratio <= 0.8 {
                    2
                } else {
                    1
                };
            }
        }
    }

    GpuStatus {
        gpus,
        model_vram_bytes,
        suggested_concurrency,
    }
}

async fn ollama_ps_vram(config: &LlmConfig) -> Option<u64> {
    let client = build_client(config).ok()?;
    let url = endpoint(&ollama_root(&config.base_url), "api/ps");
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    #[derive(Deserialize)]
    struct Ps {
        #[serde(default)]
        models: Vec<PsModel>,
    }
    #[derive(Deserialize)]
    struct PsModel {
        #[serde(default)]
        name: String,
        #[serde(default)]
        size_vram: u64,
    }
    let parsed: Ps = resp.json().await.ok()?;
    parsed
        .models
        .iter()
        .find(|m| m.name == config.model || m.name.starts_with(&config.model))
        .map(|m| m.size_vram)
        .or_else(|| parsed.models.first().map(|m| m.size_vram))
}

/// 查询引擎当前“已加载/常驻”的模型（尽力而为，失败返回空）。
pub async fn list_loaded_models(config: &LlmConfig) -> AppResult<Vec<ModelInfo>> {
    let client = build_client(config)?;
    match config.engine {
        EngineKind::Ollama => {
            let url = endpoint(&ollama_root(&config.base_url), "api/ps");
            let resp = match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r,
                _ => return Ok(Vec::new()),
            };
            #[derive(Deserialize)]
            struct Ps {
                #[serde(default)]
                models: Vec<PsModel>,
            }
            #[derive(Deserialize)]
            struct PsModel {
                name: String,
            }
            let parsed: Ps = resp.json().await.unwrap_or(Ps { models: vec![] });
            Ok(parsed
                .models
                .into_iter()
                .map(|m| ModelInfo {
                    id: m.name,
                    owned_by: Some("loaded".into()),
                })
                .collect())
        }
        _ => {
            // LM Studio 原生接口：/api/v0/models（若不可用则返回空）。
            let url = endpoint(
                config
                    .base_url
                    .trim_end_matches('/')
                    .trim_end_matches("/v1"),
                "api/v0/models",
            );
            let resp = match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r,
                _ => return Ok(Vec::new()),
            };
            #[derive(Deserialize)]
            struct V0 {
                #[serde(default)]
                data: Vec<V0Model>,
            }
            #[derive(Deserialize)]
            struct V0Model {
                id: String,
                #[serde(default)]
                state: Option<String>,
            }
            let parsed: V0 = resp.json().await.unwrap_or(V0 { data: vec![] });
            Ok(parsed
                .data
                .into_iter()
                .filter(|m| m.state.as_deref() == Some("loaded"))
                .map(|m| ModelInfo {
                    id: m.id,
                    owned_by: Some("loaded".into()),
                })
                .collect())
        }
    }
}

/// 切换模型：卸载上一个（若支持），并预热加载当前模型。
/// 返回可读的状态说明；所有底层调用均为尽力而为，不因引擎差异而报错。
pub async fn activate_model(config: &LlmConfig, previous: Option<&str>) -> AppResult<String> {
    if previous == Some(config.model.as_str()) {
        return Ok(format!("模型 {} 已就绪", config.model));
    }
    let client = build_client(config)?;

    match config.engine {
        EngineKind::Ollama => {
            let root = ollama_root(&config.base_url);
            if let Some(prev) = previous {
                let _ = client
                    .post(endpoint(&root, "api/generate"))
                    .json(&json!({ "model": prev, "keep_alive": 0 }))
                    .send()
                    .await;
            }
            // 预热：触发加载并保持 10 分钟。
            let warm = client
                .post(endpoint(&root, "api/generate"))
                .json(&json!({ "model": config.model, "keep_alive": "10m" }))
                .send()
                .await;
            match warm {
                Ok(r) if r.status().is_success() => {
                    Ok(format!("已加载模型 {}（已卸载上一个）", config.model))
                }
                _ => Ok(format!(
                    "已切换为 {}；Ollama 将在首次请求时加载",
                    config.model
                )),
            }
        }
        _ => {
            // LM Studio：通过一次极小的补全触发即时（JIT）加载。
            let req = ChatRequest {
                model: &config.model,
                messages: vec![ChatMessage {
                    role: "user",
                    content: "ok",
                }],
                temperature: 0.0,
                max_tokens: 1,
                stream: false,
                response_format: None,
                format: None,
                reasoning_effort: None,
            };
            let warmed = apply_auth(
                client.post(&endpoint(&config.base_url, "chat/completions")),
                config,
            )
            .json(&req)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
            Ok(if warmed {
                format!("已加载模型 {}", config.model)
            } else {
                format!("已切换为 {}；请在引擎中确认该模型已加载", config.model)
            })
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request(reasoning_effort: Option<&str>) -> ChatRequest<'_> {
        ChatRequest {
            model: "test-model",
            messages: vec![ChatMessage {
                role: "user",
                content: "hi",
            }],
            temperature: 0.0,
            max_tokens: 16,
            stream: false,
            response_format: Some(json!({ "type": "json_object" })),
            format: None,
            reasoning_effort,
        }
    }

    #[test]
    fn reasoning_effort_is_sent_as_none() {
        let value = serde_json::to_value(sample_request(Some("none"))).unwrap();
        assert_eq!(value["reasoning_effort"], "none");
    }

    #[test]
    fn reasoning_effort_is_omitted_when_disabled() {
        let value = serde_json::to_value(sample_request(None)).unwrap();
        assert!(value.get("reasoning_effort").is_none());
    }
}
