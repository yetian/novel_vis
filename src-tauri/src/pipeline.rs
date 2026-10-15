use crate::error::{AppError, AppResult};
use crate::llm;
use crate::models::*;
use crate::state::{AppState, JobControl};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub const EVENT_PROGRESS: &str = "extraction://progress";
pub const EVENT_LOG: &str = "extraction://log";

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractionRequest {
    pub book_id: i64,
    pub llm: LlmConfig,
    #[serde(default)]
    pub chunk_chars: Option<usize>,
    #[serde(default)]
    pub skip_extracted: bool,
}

static TRAILING_COMMA_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r",(\s*[}\]])").unwrap());
static LINE_COMMENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)//[^\n]*").unwrap());
static BLOCK_COMMENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());

/// 抽取管线主入口：逐章 -> 逐块调用本地大模型，产出结构化实体并持久化。
pub async fn run_extraction(
    app: AppHandle,
    state: &AppState,
    req: ExtractionRequest,
) -> AppResult<Vec<ChapterExtractionResult>> {
    let book = state
        .book(req.book_id)
        .ok_or_else(|| AppError::Internal("书籍原文不在内存中，请重新导入后再抽取。".into()))?;

    let db = state.db.clone();
    let control = state.register_job(req.book_id);
    let total = book.chapters.len();
    let chunk_chars = req.chunk_chars.unwrap_or(3000).clamp(800, 12_000);

    let mut results: Vec<ChapterExtractionResult> = Vec::with_capacity(total);

    for (pos, chapter) in book.chapters.iter().enumerate() {
        if control.is_cancelled() {
            emit_log(&app, req.book_id, "任务已取消");
            break;
        }
        await_resume(&app, &control, req.book_id).await;

        let chapter_id = db.insert_chapter(
            req.book_id,
            chapter.chapter_index,
            &chapter.title,
            &chapter.content_hash,
            None,
        )?;

        if req.skip_extracted && db.chapter_extracted(chapter_id)? {
            emit_progress(
                &app,
                &req.book_id,
                "skipped",
                pos + 1,
                total,
                chapter.chapter_index,
                &chapter.title,
                "已存在抽取结果，跳过",
                0,
            );
            results.push(ChapterExtractionResult {
                chapter_index: chapter.chapter_index,
                chapter_title: chapter.title.clone(),
                ok: true,
                retries_used: 0,
                repaired: false,
                extraction: None,
                error: None,
            });
            continue;
        }

        let chunks = split_chunks(&chapter.text, chunk_chars);
        let chunk_total = chunks.len().max(1);

        let mut chapter_ok = true;
        let mut retries_total = 0u32;
        let mut repaired = false;
        let mut first_ext: Option<SegmentExtraction> = None;
        let mut first_error: Option<String> = None;

        for (ci, chunk) in chunks.iter().enumerate() {
            if control.is_cancelled() {
                break;
            }
            await_resume(&app, &control, req.book_id).await;

            emit_progress(
                &app,
                &req.book_id,
                "extracting",
                pos,
                total,
                chapter.chapter_index,
                &chapter.title,
                &format!("正在抽取第 {} 章 · 分块 {}/{}", chapter.chapter_index + 1, ci + 1, chunk_total),
                0,
            );

            match extract_chunk(&req.llm, chapter, ci, chunk_total, chunk, &app, req.book_id).await {
                Ok((ext, used, was_repaired)) => {
                    retries_total += used;
                    repaired |= was_repaired;
                    if first_ext.is_none() {
                        first_ext = Some(ext.clone());
                    }
                    if let Err(e) = db.persist_extraction(req.book_id, chapter_id, &ext) {
                        chapter_ok = false;
                        first_error = Some(e.to_string());
                    }
                }
                Err(e) => {
                    chapter_ok = false;
                    first_error = Some(e.to_string());
                    emit_log(
                        &app,
                        req.book_id,
                        &format!("第 {} 章分块 {} 抽取失败: {}", chapter.chapter_index + 1, ci + 1, e),
                    );
                }
            }
        }

        emit_progress(
            &app,
            &req.book_id,
            "chapter_done",
            pos + 1,
            total,
            chapter.chapter_index,
            &chapter.title,
            if chapter_ok { "章节完成" } else { "章节存在失败分块" },
            retries_total,
        );

        results.push(ChapterExtractionResult {
            chapter_index: chapter.chapter_index,
            chapter_title: chapter.title.clone(),
            ok: chapter_ok,
            retries_used: retries_total,
            repaired,
            extraction: first_ext,
            error: first_error,
        });
    }

    emit_progress(
        &app,
        &req.book_id,
        if control.is_cancelled() { "cancelled" } else { "finished" },
        total,
        total,
        0,
        "",
        if control.is_cancelled() { "任务已取消（进度已保存）" } else { "全部章节抽取完成" },
        0,
    );
    state.clear_job(req.book_id);
    Ok(results)
}

async fn extract_chunk(
    llm_config: &LlmConfig,
    chapter: &ParsedChapter,
    chunk_index: usize,
    chunk_total: usize,
    text: &str,
    app: &AppHandle,
    book_id: i64,
) -> AppResult<(SegmentExtraction, u32, bool)> {
    let user_prompt = build_user_prompt(chapter, chunk_index, chunk_total, text);
    let mut last_error = String::new();
    let mut last_output = String::new();
    let mut retries = 0u32;

    for attempt in 0..=llm_config.max_retries {
        if attempt > 0 {
            retries += 1;
            emit_log(
                app,
                book_id,
                &format!(
                    "第 {} 章 JSON 修复重试 {}/{}",
                    chapter.chapter_index + 1,
                    attempt,
                    llm_config.max_retries
                ),
            );
        }

        let prompt = if attempt == 0 || last_output.is_empty() {
            user_prompt.clone()
        } else {
            build_repair_prompt(&user_prompt, &last_output, &last_error)
        };

        let response = match llm::chat_json(llm_config, SYSTEM_PROMPT, &prompt, true).await {
            Ok(content) => content,
            Err(e) => {
                last_error = e.to_string();
                continue;
            }
        };

        match parse_extraction(&response) {
            Ok((ext, locally_repaired)) => {
                return Ok((ext, retries, locally_repaired || attempt > 0));
            }
            Err(e) => {
                last_error = e.to_string();
                last_output = response;
            }
        }
    }

    Err(AppError::JsonRepair(format!(
        "模型输出无法修复（已重试 {} 次）: {}",
        retries, last_error
    )))
}

/// 从模型输出中解析出 SegmentExtraction，包含本地 JSON 修复策略。
pub fn parse_extraction(raw: &str) -> AppResult<(SegmentExtraction, bool)> {
    let block = extract_json_block(raw)
        .ok_or_else(|| AppError::JsonRepair("输出中未找到 JSON 对象".into()))?;

    let mut repaired = false;
    let mut value: Value = match serde_json::from_str(&block) {
        Ok(v) => v,
        Err(_) => {
            repaired = true;
            let fixed = repair_json_text(&block);
            serde_json::from_str(&fixed)
                .map_err(|e| AppError::JsonRepair(format!("JSON 解析失败: {e}")))?
        }
    };

    normalize_value(&mut value);

    match serde_json::from_value::<SegmentExtraction>(value.clone()) {
        Ok(ext) => Ok((ext, repaired)),
        Err(_) => Ok((lenient_extraction(&value), repaired)),
    }
}

/// 提取首个「平衡」的 JSON 对象：正确处理字符串与转义，
/// 因此可容忍前后缀文本、Markdown 代码块，以及被 `[ ... ]` 包裹的对象。
fn extract_json_block(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in raw[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(raw[start..start + offset + ch.len_utf8()].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn repair_json_text(input: &str) -> String {
    let no_block = BLOCK_COMMENT_RE.replace_all(input, "");
    let no_line = LINE_COMMENT_RE.replace_all(&no_block, "");
    let no_trailing = TRAILING_COMMA_RE.replace_all(&no_line, "$1");
    escape_control_chars_in_strings(&no_trailing)
}

/// 转义字符串字面量内的裸控制字符（\u0000-\u001F）。
/// 小模型常在 JSON 字符串里直接输出换行/制表符，导致 serde_json 报
/// “control character found while parsing a string”。
fn escape_control_chars_in_strings(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    let mut in_string = false;
    let mut escaped = false;
    for ch in input.chars() {
        if in_string {
            if escaped {
                out.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => {
                    out.push('\\');
                    escaped = true;
                }
                '"' => {
                    out.push('"');
                    in_string = false;
                }
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        } else {
            if ch == '"' {
                in_string = true;
            }
            out.push(ch);
        }
    }
    out
}

fn normalize_value(value: &mut Value) {
    let obj = match value.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    for key in ["characters", "items_and_skills"] {
        if !obj.get(key).map(Value::is_array).unwrap_or(false) {
            obj.insert(key.to_string(), Value::Array(vec![]));
        }
    }

    if let Some(Value::Array(chars)) = obj.get_mut("characters") {
        chars.retain(|c| c.get("name").and_then(Value::as_str).map(|s| !s.trim().is_empty()).unwrap_or(false));
    }
    if let Some(Value::Array(items)) = obj.get_mut("items_and_skills") {
        items.retain(|c| c.get("name").and_then(Value::as_str).map(|s| !s.trim().is_empty()).unwrap_or(false));
    }
}

/// 当严格反序列化失败时的兜底策略：尽可能保留可识别字段。
fn lenient_extraction(value: &Value) -> SegmentExtraction {
    let get_str = |k: &str| value.get(k).and_then(Value::as_str).map(|s| s.to_string());
    let temporal = value
        .get("temporal")
        .and_then(|t| serde_json::from_value::<Temporal>(t.clone()).ok());
    let spatial = value
        .get("spatial")
        .and_then(|t| serde_json::from_value::<Spatial>(t.clone()).ok());
    SegmentExtraction {
        chapter_index: value.get("chapter_index").and_then(Value::as_i64).unwrap_or(0),
        chapter_title: get_str("chapter_title").unwrap_or_default(),
        narrative_perspective: get_str("narrative_perspective"),
        temporal,
        spatial,
        characters: vec![],
        items_and_skills: vec![],
        plot_summary: get_str("plot_summary").unwrap_or_default(),
    }
}

fn split_chunks(text: &str, target: usize) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for para in text.split('\n') {
        if para.chars().count() > target {
            if !current.trim().is_empty() {
                chunks.push(current.trim().to_string());
                current = String::new();
            }
            for piece in hard_split(para, target) {
                chunks.push(piece);
            }
            continue;
        }
        if current.chars().count() + para.chars().count() + 1 > target && !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
            current = String::new();
        }
        current.push_str(para);
        current.push('\n');
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }
    if chunks.is_empty() {
        chunks.push(text.to_string());
    }
    chunks
}

fn hard_split(text: &str, target: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(target)
        .map(|c| c.iter().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn build_user_prompt(chapter: &ParsedChapter, chunk_index: usize, chunk_total: usize, text: &str) -> String {
    format!(
        "【章节序号】{}\n【章节标题】{}\n【分块】{}/{}\n\n【正文】\n{}\n\n请严格按系统提示的 JSON schema 输出该分块的抽取结果，只返回 JSON。",
        chapter.chapter_index,
        chapter.title,
        chunk_index + 1,
        chunk_total,
        text
    )
}

fn build_repair_prompt(original: &str, bad_output: &str, error: &str) -> String {
    format!(
        "上一次输出不是合法的、符合 schema 的 JSON。\n\n【解析错误】{}\n\n【你的上一次输出】\n{}\n\n【原始任务】\n{}\n\n请仅返回修复后的、严格合法的 JSON 对象，不要包含任何解释或 Markdown 代码块。",
        error,
        truncate(bad_output, 4000),
        truncate(original, 4000)
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

async fn await_resume(app: &AppHandle, control: &Arc<JobControl>, book_id: i64) {
    while control.is_paused() && !control.is_cancelled() {
        emit_progress(app, &book_id, "paused", 0, 0, 0, "", "任务已暂停", 0);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_progress(
    app: &AppHandle,
    book_id: &i64,
    phase: &str,
    current: usize,
    total: usize,
    chapter_index: i64,
    chapter_title: &str,
    message: &str,
    retries: u32,
) {
    Emitter::emit(
        app,
        EVENT_PROGRESS,
        &ProgressEvent {
            book_id: *book_id,
            phase: phase.to_string(),
            current,
            total,
            chapter_index,
            chapter_title: chapter_title.to_string(),
            message: message.to_string(),
            retries,
        },
    )
    .ok();
}

fn emit_log(app: &AppHandle, book_id: i64, message: &str) {
    Emitter::emit(
        app,
        EVENT_LOG,
        &LogEvent {
            book_id,
            message: message.to_string(),
        },
    )
    .ok();
}

/// 系统提示词：约束模型输出严格的时空/人物/因果 JSON。
pub const SYSTEM_PROMPT: &str = r#"你是一个中文小说叙事结构分析引擎。你的唯一任务是把输入的小说正文分块，抽取其中的【时间】【空间】【人物】【物品/功法/传承】【因果】信息，并严格输出一个 JSON 对象。

## 叙事视角归一化
- 判断本段是 first_person（第一人称，如“我拔出长剑”）还是 third_person（第三人称，如“韩立飞身而起”）。
- 若为第一人称，把“我”解析为主角的规范名称；若无法确定主角名称，则使用固定标识 "PROTAGONIST_FIRST_PERSON"，并在人物 description 中给出该视角下的上下文别名/称谓。
- 人物 name 一律使用全书中最规范、最常出现的称谓（如“韩立”“墨大夫”），不要使用“他”“那人”等代词。

## 输出 JSON Schema（字段必须齐全，枚举值必须完全一致）
{
  "chapter_index": 整数,
  "chapter_title": "字符串",
  "narrative_perspective": "first_person" | "third_person",
  "temporal": {
    "type": "CONTINUOUS" | "JUMP_FORWARD" | "FLASHBACK" | "UNCERTAIN",
    "raw_marker": "原文时间标记，如 三年后 / 眨眼数日",
    "relative_delta_days": 整数（换算为天数，无则 0）,
    "reference_anchor": "所参照的上一事件描述"
  },
  "spatial": {
    "world": "世界层级，如 人界 / 灵界",
    "region": "大区，如 越国",
    "site": "具体地点，如 神手谷",
    "movement_type": "STAY" | "LOCAL_MOVE" | "TELEPORT" | "ASCENSION" | "DUNGEON_ENTER" | "DUNGEON_EXIT",
    "source_site": "若发生移动，填写移动起点，否则为空字符串"
  },
  "characters": [
    { "name": "人物规范名", "action": "FIRST_APPEARANCE" | "ACTIVE" | "DEPARTURE" | "DEATH", "role": "师尊/对手/同伴等", "description": "外貌或身份描述" }
  ],
  "items_and_skills": [
    { "name": "物品或功法名", "category": "ITEM" | "SKILL" | "LEGACY", "acquired_from": { "spatial": "获得地点", "temporal": "获得时间" }, "usage_in_chapter": "本章中的用途", "future_hook_flag": true 或 false }
  ],
  "plot_summary": "一句话核心剧情摘要"
}

## 硬性要求
1. 只输出 JSON，对象之外不得有任何文字、注释或 Markdown 代码块。
2. 所有字符串用双引号；不要出现尾随逗号。
3. 无法确定的信息使用空字符串或 0，不要省略字段。
4. 《凡人修仙传》等修仙小说的世界层级请合理区分（如 人界 / 灵界 / 仙界）。
5. 物品若具有长期伏笔价值（如“掌天瓶”），future_hook_flag 设为 true。
6. 所有自然语言字段（人物名、地点、角色、描述、用途、摘要、raw_marker、reference_anchor 等）一律使用【简体中文】输出；仅 JSON 的键名与机器枚举值（如 CONTINUOUS / FIRST_APPEARANCE / ITEM）保持英文。
7. 专有名词保留原作写法，不要翻译或音译（如“韩立”“掌天瓶”“越国”“神手谷”）。
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn extract_json_block_strips_text_and_fences() {
        let raw = "好的，结果如下：\n```json\n{\"a\":1}\n```\n完成";
        assert_eq!(extract_json_block(raw).unwrap(), "{\"a\":1}");
        assert!(extract_json_block("没有任何括号").is_none());
    }

    #[test]
    fn repair_json_text_removes_comments_and_trailing_commas() {
        let broken = "{ // 注释\n \"a\": 1, \n }";
        let fixed = repair_json_text(broken);
        let v: Value = serde_json::from_str(&fixed).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn parse_extraction_valid_payload() {
        let payload = json!({
            "chapter_index": 12,
            "chapter_title": "七玄深谷",
            "narrative_perspective": "third_person",
            "temporal": {"type":"JUMP_FORWARD","raw_marker":"三年后","relative_delta_days":1095,"reference_anchor":"入门"},
            "spatial": {"world":"人界","region":"越国","site":"神手谷","movement_type":"DUNGEON_ENTER","source_site":"广场"},
            "characters":[{"name":"韩立","action":"FIRST_APPEARANCE","role":"主角","description":"少年"}],
            "items_and_skills":[{"name":"掌天瓶","category":"ITEM","acquired_from":{"spatial":"后山","temporal":"入夜"},"usage_in_chapter":"拾取","future_hook_flag":true}],
            "plot_summary":"韩立拾得掌天瓶"
        })
        .to_string();

        let (ext, repaired) = parse_extraction(&payload).unwrap();
        assert!(!repaired);
        assert_eq!(ext.chapter_index, 12);
        assert_eq!(ext.temporal.as_ref().unwrap().relative_delta_days, Some(1095));
        assert_eq!(ext.spatial.as_ref().unwrap().world.as_deref(), Some("人界"));
        assert_eq!(ext.characters.len(), 1);
        assert_eq!(ext.characters[0].action, "FIRST_APPEARANCE");
        assert!(ext.items_and_skills[0].future_hook_flag);
    }

    #[test]
    fn parse_extraction_repairs_trailing_comma() {
        let broken =
            r#"{"chapter_index":1,"characters":[{"name":"韩立"}],"items_and_skills":[],}"#;
        let (ext, repaired) = parse_extraction(broken).unwrap();
        assert!(repaired);
        assert_eq!(ext.characters.len(), 1);
        assert_eq!(ext.characters[0].name, "韩立");
    }

    #[test]
    fn parse_extraction_normalizes_first_person() {
        let payload = json!({
            "narrative_perspective": "first_person",
            "characters": [{"name": "PROTAGONIST_FIRST_PERSON", "action": "ACTIVE"}],
            "items_and_skills": []
        })
        .to_string();
        let (ext, _) = parse_extraction(&payload).unwrap();
        assert_eq!(ext.narrative_perspective.as_deref(), Some("first_person"));
        assert_eq!(ext.characters[0].name, "PROTAGONIST_FIRST_PERSON");
    }

    #[test]
    fn parse_extraction_drops_nameless_entries() {
        let payload = r#"{"characters":[{"action":"ACTIVE"},{"name":"韩立"}],"items_and_skills":[{"category":"ITEM"}]}"#;
        let (ext, _) = parse_extraction(payload).unwrap();
        assert_eq!(ext.characters.len(), 1);
        assert_eq!(ext.characters[0].name, "韩立");
        assert!(ext.items_and_skills.is_empty());
    }

    #[test]
    fn parse_extraction_errors_without_json() {
        assert!(parse_extraction("完全没有 JSON 对象").is_err());
    }

    #[test]
    fn extract_json_block_handles_array_and_trailing_text() {
        assert_eq!(extract_json_block("结果：[ {\"a\":1}, {\"b\":2} ] 完").unwrap(), "{\"a\":1}");
        assert_eq!(
            extract_json_block("{\"temporal\":{\"type\":\"CONTINUOUS\"}}尾").unwrap(),
            "{\"temporal\":{\"type\":\"CONTINUOUS\"}}"
        );
    }

    #[test]
    fn parse_extraction_repairs_raw_control_chars() {
        // 字符串内直接包含换行/制表符（未转义）——小模型常见错误。
        let broken = "{\"plot_summary\":\"第一行\n第二行\t结束\",\"characters\":[],\"items_and_skills\":[]}";
        let (ext, repaired) = parse_extraction(broken).unwrap();
        assert!(repaired);
        assert!(ext.plot_summary.contains("第一行"));
        assert!(ext.plot_summary.contains("第二行"));
    }

    #[test]
    fn split_chunks_splits_long_text_without_empty_chunks() {
        let text = format!("{}\n{}", "甲".repeat(2000), "乙".repeat(2000));
        let chunks = split_chunks(&text, 3000);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|c| !c.trim().is_empty()));
    }

    #[test]
    fn split_chunks_keeps_short_text_single() {
        assert_eq!(split_chunks("短文本", 3000).len(), 1);
    }
}
