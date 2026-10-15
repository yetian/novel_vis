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
    format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
}

fn apply_auth(
    req: reqwest::RequestBuilder,
    config: &LlmConfig,
) -> reqwest::RequestBuilder {
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
pub async fn chat_json(
    config: &LlmConfig,
    system: &str,
    user: &str,
    json_mode: bool,
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
    };

    let resp = apply_auth(client.post(&url), config)
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::Llm(format!("请求 {url} 失败: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // 某些引擎不支持 response_format，降级重试一次。
        if json_mode {
            return Box::pin(chat_json(config, system, user, false)).await;
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
