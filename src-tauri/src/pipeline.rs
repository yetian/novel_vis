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
    /// 并发请求数（1–4）。需引擎支持并行/批处理才能获得加速。
    #[serde(default)]
    pub concurrency: Option<u32>,
}

static TRAILING_COMMA_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r",(\s*[}\]])").unwrap());
static LINE_COMMENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)//[^\n]*").unwrap());
static BLOCK_COMMENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());
// 值之后、下一个字符串键之前缺少逗号（常见于小模型逐行输出）。
static MISSING_COMMA_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"([}\]"]|true|false|null|[0-9])(\s*)("[A-Za-z_\u4e00-\u9fff])"#).unwrap()
});

/// 抽取管线主入口：按并发度成批处理章节，逐块调用本地大模型并持久化。
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
    // 0 = 整章输入（不切块）；否则限制在合理区间。
    let chunk_chars = match req.chunk_chars.unwrap_or(2000) {
        0 => 0,
        v => v.clamp(800, 200_000),
    };
    let concurrency = req.concurrency.unwrap_or(1).clamp(1, 4) as usize;

    // 未勾选「跳过已完成」= 重新处理整本书，先清除旧的抽取结果以避免重复。
    if !req.skip_extracted {
        db.clear_book_extraction(req.book_id)?;
    }
    if concurrency > 1 {
        emit_log(
            &app,
            req.book_id,
            &format!("启用 {concurrency} 路并发抽取（需引擎支持并行/批处理才会真正提速）"),
        );
    }

    let chapters: Vec<ParsedChapter> = book.chapters.clone();
    let mut results: Vec<ChapterExtractionResult> = Vec::with_capacity(total);
    let mut next = 0usize;
    // 滑动窗口：完成一个任务就补一个，避免某个章节的 JSON 重试拖住整批。
    let mut set: tokio::task::JoinSet<ChapterExtractionResult> = tokio::task::JoinSet::new();

    loop {
        if control.is_cancelled() {
            emit_log(&app, req.book_id, "任务已取消");
            break;
        }
        await_resume(&app, &control, req.book_id).await;

        while set.len() < concurrency && next < chapters.len() {
            let cp = next;
            let chapter = chapters[next].clone();
            next += 1;
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
                    cp + 1,
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

            let task_app = app.clone();
            let task_db = db.clone();
            let task_llm = req.llm.clone();
            let task_control = control.clone();
            let book_id = req.book_id;
            set.spawn(async move {
                extract_chapter(
                    task_app,
                    task_db,
                    task_llm,
                    book_id,
                    chapter,
                    chapter_id,
                    chunk_chars,
                    task_control,
                    cp,
                    total,
                )
                .await
            });
        }

        if set.is_empty() {
            break;
        }

        if let Some(joined) = set.join_next().await {
            match joined {
                Ok(r) => results.push(r),
                Err(e) => emit_log(&app, req.book_id, &format!("并发任务失败: {e}")),
            }
        }
    }

    results.sort_by_key(|r| r.chapter_index);

    // 只有整本书全部成功处理后才生成全书总览，避免用残缺数据误导。
    let completed =
        !control.is_cancelled() && results.len() == total && results.iter().all(|r| r.ok);
    if completed {
        emit_progress(
            &app,
            &req.book_id,
            "summarizing",
            total,
            total,
            0,
            "",
            "全部章节抽取完成，正在生成全书总览…",
            0,
        );
        match summarize_book(
            &req.llm,
            req.book_id,
            &book.title,
            book.author.as_deref(),
            &db,
        )
        .await
        {
            Ok(Some(summary)) => {
                db.set_book_summary(req.book_id, &summary)?;
                emit_log(
                    &app,
                    req.book_id,
                    &format!("全书总览已生成（{} 字）", summary.chars().count()),
                );
            }
            Ok(None) => emit_log(&app, req.book_id, "没有可用的章节摘要，跳过全书总览"),
            Err(e) => emit_log(&app, req.book_id, &format!("全书总览生成失败: {e}")),
        }
    }

    emit_progress(
        &app,
        &req.book_id,
        if control.is_cancelled() {
            "cancelled"
        } else {
            "finished"
        },
        total,
        total,
        0,
        "",
        if control.is_cancelled() {
            "任务已取消（进度已保存）"
        } else {
            "全部章节抽取完成"
        },
        0,
    );
    state.clear_job(req.book_id);
    Ok(results)
}

const BOOK_SUMMARY_SYSTEM: &str = r#"你是资深中文小说编辑。你只输出一个 JSON 对象，不得输出解释、Markdown 或代码块。
输出格式：只包含一个 summary 字段，值为你撰写的总览正文。
summary 中只放总览正文：不要写标题、“全书总览”等字样、书名/作者、分点、小标题、章节列表、字数说明、免责声明，也不要写“好的”“以下是”等前言后语；只写连贯的正文段落。
要求：只依据用户提供的章节/阶段摘要写作；不要编造未给出的情节；用简体中文；语言简洁、连贯、有概括力；保留关键人物与地点的原文名称。
summary 必须是完整正文，不得只写省略号、占位符或复述材料；长度按用户要求。"#;

/// 用章节摘要生成全书总览。章节很多时先分批生成阶段摘要，再汇总最终总览。
pub async fn summarize_book(
    config: &LlmConfig,
    book_id: i64,
    title: &str,
    author: Option<&str>,
    db: &crate::db::Db,
) -> AppResult<Option<String>> {
    let chapters = db.get_chapters(book_id)?;
    let mut lines = Vec::new();
    for c in &chapters {
        let Some(summary) = c
            .raw_text_summary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let summary: String = summary.chars().take(140).collect();
        let chapter_title = c.title.trim();
        if chapter_title.is_empty() {
            lines.push(format!("第 {} 章：{summary}", c.chapter_index + 1));
        } else {
            lines.push(format!(
                "第 {} 章 {chapter_title}：{summary}",
                c.chapter_index + 1
            ));
        }
    }
    if lines.is_empty() {
        return Ok(None);
    }

    // 控制单次上下文规模，兼容 8k 上下文的本地模型；过长时自动分批。
    let batches = pack_summary_lines(&lines, 5_000);
    let staged = batches.len() > 1;
    let parts = if !staged {
        batches
    } else {
        let mut stage_summaries = Vec::with_capacity(batches.len());
        for (i, batch) in batches.iter().enumerate() {
            let prompt = format!(
                "以下是《{}》第 {}/{} 组的章节摘要。请只根据这一组材料，用 180-260 字连贯概括本阶段的剧情推进、主要人物与地点变化。summary 只写正文：不要标题、书名/作者、前言、分点、章节列表；不要提前概括后续，不要复述材料，保留关键专名。只输出 JSON，summary 字段填入完整正文，不要写省略号或占位符。\n\n{}",
                title,
                i + 1,
                batches.len(),
                batch
            );
            stage_summaries.push(call_summary(config, &prompt).await?);
        }
        stage_summaries
    };

    let author_line = author
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(|a| format!("作者：{a}"))
        .unwrap_or_default();
    let material = parts.join("\n\n");
    let scope = if staged {
        "以下是全书各阶段的摘要"
    } else {
        "以下是全书的章节摘要"
    };
    let prompt = format!(
        "《{title}》{author_line}\n{scope}。书名与作者仅用于识别作品，不要写进 summary。请写一段 300-500 字的全书总览正文，要求：\n1. 概括核心设定、世界/空间与主线冲突；\n2. 梳理开端—发展—转折—结局（若材料未覆盖结局，写到当前进展，不要猜测）；\n3. 点出 3-8 位关键人物及其关系或作用；\n4. 只依据下面材料，不编造未出现的情节，保留关键人物/地点的原文名称；\n5. 不分点、不复述章节列表，写成连贯的段落；\n6. summary 中只放正文，不要标题、“全书总览”等字样、书名/作者、前言、说明或章节列表。\n只输出 JSON，summary 字段填入完整总览正文，不要写省略号或占位符。\n\n{}",
        material
    );
    let summary = call_summary(config, &prompt).await?;
    Ok(Some(summary))
}

/// 按字符预算把章节摘要打包成若干组，保持章节顺序。
pub fn pack_summary_lines(lines: &[String], max_chars: usize) -> Vec<String> {
    let budget = max_chars.max(200);
    let mut out = Vec::new();
    let mut current = String::new();
    for line in lines {
        let line: String = line.chars().take(budget).collect();
        if line.trim().is_empty() {
            continue;
        }
        if !current.is_empty() && current.chars().count() + line.chars().count() + 1 > budget {
            out.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(&line);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

async fn call_summary(config: &LlmConfig, prompt: &str) -> AppResult<String> {
    let mut cfg = config.clone();
    cfg.max_tokens = cfg.max_tokens.max(1_400);
    let raw = llm::chat_json(&cfg, BOOK_SUMMARY_SYSTEM, prompt, true).await?;
    if let Some(summary) = parse_summary_output(&raw) {
        if summary_is_usable(&summary) {
            return Ok(summary);
        }
    }

    // 小模型容易把提示里的占位符原样抄回来；改用纯文本再试一次。
    let retry_system = "你是资深中文小说编辑。请直接输出总览正文，只写正文；不要 JSON、标题、书名/作者、前言、说明、分点、Markdown、省略号或占位符。";
    let json_sentence = "只输出 JSON，summary 字段填入完整正文，不要写省略号或占位符。";
    let json_sentence_long = "只输出 JSON，summary 字段填入完整总览正文，不要写省略号或占位符。";
    let plain_sentence =
        "请直接输出总览正文：不要标题、书名/作者、JSON、解释、Markdown、分点、省略号或占位符。";
    let mut retry_prompt = prompt
        .replace(json_sentence, plain_sentence)
        .replace(json_sentence_long, plain_sentence);
    if retry_prompt == prompt {
        retry_prompt = format!("{prompt}\n\n{plain_sentence}");
    }
    let retry_raw = llm::chat_json(&cfg, retry_system, &retry_prompt, false).await?;
    let summary = parse_summary_output(&retry_raw).unwrap_or_default();
    if summary_is_usable(&summary) {
        return Ok(summary);
    }
    Err(AppError::Llm(format!(
        "全书总览内容无效（{} 字），模型可能只输出了占位符：{}",
        summary.chars().count(),
        truncate(&retry_raw, 160)
    )))
}

/// 过滤 “...”“……”“待生成” 之类占位符，避免把无效内容写进数据库。
fn summary_is_usable(summary: &str) -> bool {
    let meaningful = summary
        .trim()
        .chars()
        .filter(|c| {
            !c.is_whitespace()
                && !c.is_ascii_punctuation()
                && !matches!(
                    c,
                    '…' | '。'
                        | '，'
                        | '、'
                        | '；'
                        | '：'
                        | '！'
                        | '？'
                        | '“'
                        | '”'
                        | '‘'
                        | '’'
                        | '（'
                        | '）'
                        | '《'
                        | '》'
                        | '—'
                        | '·'
                        | '～'
                        | '~'
                )
        })
        .count();
    meaningful >= 20
}

fn parse_summary_output(raw: &str) -> Option<String> {
    if let Some(block) = extract_json_block(raw) {
        if let Ok(value) = serde_json::from_str::<Value>(&block) {
            if let Some(summary) = value.get("summary").and_then(Value::as_str) {
                let summary = summary.trim();
                if !summary.is_empty() {
                    return Some(summary.to_string());
                }
            }
        }
    }
    let text = raw.trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text);
    let text = text.strip_suffix("```").unwrap_or(text).trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// 抽取单个章节（含其所有分块）。作为并发任务单元运行。
#[allow(clippy::too_many_arguments)]
async fn extract_chapter(
    app: AppHandle,
    db: Arc<crate::db::Db>,
    llm_config: LlmConfig,
    book_id: i64,
    chapter: ParsedChapter,
    chapter_id: i64,
    chunk_chars: usize,
    control: Arc<JobControl>,
    pos: usize,
    total: usize,
) -> ChapterExtractionResult {
    let chunks = chapter_chunks(&chapter.text, chunk_chars);
    let chunk_total = chunks.len().max(1);

    let mut chapter_ok = true;
    let mut retries_total = 0u32;
    let mut repaired = false;
    let mut first_ext: Option<SegmentExtraction> = None;
    let mut first_error: Option<String> = None;

    for (ci, chunk) in chunks.iter().enumerate() {
        if control.is_cancelled() {
            if first_ext.is_none() && first_error.is_none() {
                first_error = Some("已取消".into());
                chapter_ok = false;
            }
            break;
        }
        await_resume(&app, &control, book_id).await;

        emit_progress(
            &app,
            &book_id,
            "extracting",
            pos,
            total,
            chapter.chapter_index,
            &chapter.title,
            &format!(
                "正在抽取第 {} 章 · 分块 {}/{}",
                chapter.chapter_index + 1,
                ci + 1,
                chunk_total
            ),
            0,
        );

        match extract_chunk(&llm_config, &chapter, ci, chunk_total, chunk, &app, book_id).await {
            Ok((ext, used, was_repaired)) => {
                retries_total += used;
                repaired |= was_repaired;
                if first_ext.is_none() {
                    first_ext = Some(ext.clone());
                }
                if let Err(e) = db.persist_extraction(book_id, chapter_id, &ext) {
                    chapter_ok = false;
                    first_error = Some(e.to_string());
                }
            }
            Err(e) => {
                chapter_ok = false;
                first_error = Some(e.to_string());
                emit_log(
                    &app,
                    book_id,
                    &format!(
                        "第 {} 章分块 {} 抽取失败: {}",
                        chapter.chapter_index + 1,
                        ci + 1,
                        e
                    ),
                );
            }
        }
    }

    emit_progress(
        &app,
        &book_id,
        "chapter_done",
        pos + 1,
        total,
        chapter.chapter_index,
        &chapter.title,
        if chapter_ok {
            "章节完成"
        } else {
            "章节存在失败分块"
        },
        retries_total,
    );

    ChapterExtractionResult {
        chapter_index: chapter.chapter_index,
        chapter_title: chapter.title.clone(),
        ok: chapter_ok,
        retries_used: retries_total,
        repaired,
        extraction: first_ext,
        error: first_error,
    }
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
    // 章节 JSON 可能包含大量人物/物品；给输出留足预算，避免中途截断。
    let mut cfg = llm_config.clone();
    cfg.max_tokens = cfg.max_tokens.max(4096);
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

        let response = match llm::chat_json(&cfg, SYSTEM_PROMPT, &prompt, true).await {
            Ok(content) => content,
            Err(e) => {
                last_error = e.to_string();
                continue;
            }
        };

        match parse_extraction(&response) {
            Ok((mut ext, locally_repaired)) => {
                let dropped = ground_entries(&mut ext, text);
                if dropped > 0 {
                    emit_log(
                        app,
                        book_id,
                        &format!(
                            "第 {} 章：已过滤 {} 个未在正文出现的实体",
                            chapter.chapter_index + 1,
                            dropped
                        ),
                    );
                }
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
    let trimmed = input.trim().trim_start_matches('\u{feff}');
    let no_block = BLOCK_COMMENT_RE.replace_all(trimmed, "");
    let no_line = LINE_COMMENT_RE.replace_all(&no_block, "");
    let no_trailing = TRAILING_COMMA_RE.replace_all(&no_line, "$1");
    let escaped = escape_control_chars_in_strings(&no_trailing);
    let with_commas = MISSING_COMMA_RE.replace_all(&escaped, "$1,$2$3");
    balance_braces(&with_commas)
}

/// 补齐被截断时缺失的闭合括号（模型输出超长被截断的常见情况）。
fn balance_braces(input: &str) -> String {
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in input.chars() {
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
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                stack.pop();
            }
            _ => {}
        }
    }
    let mut out = input.to_string();
    while let Some(closer) = stack.pop() {
        out.push(closer);
    }
    out
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
        chars.retain(|c| {
            c.get("name")
                .and_then(Value::as_str)
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
        });
        for c in chars.iter_mut() {
            normalize_character(c);
        }
    }
    if let Some(Value::Array(items)) = obj.get_mut("items_and_skills") {
        items.retain(|c| {
            c.get("name")
                .and_then(Value::as_str)
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
        });
        for it in items.iter_mut() {
            normalize_item(it);
        }
    }
    if let Some(temporal) = obj.get_mut("temporal") {
        normalize_temporal(temporal);
    }
    if let Some(spatial) = obj.get_mut("spatial") {
        normalize_spatial(spatial);
    }
    normalize_enum_field(
        obj,
        "narrative_perspective",
        &["first_person", "third_person"],
        "third_person",
        false,
    );
}

/// importance 容忍字符串/浮点，并夹紧到 1–5，避免小模型格式偏差导致整段解析失败。
fn normalize_importance(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if let Some(slot) = obj.get_mut("importance") {
        let parsed = match slot {
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
            Value::String(s) => s.trim().parse::<f64>().ok().map(|f| f.round() as i64),
            _ => None,
        };
        *slot = match parsed {
            Some(n) => Value::from(n.clamp(1, 5)),
            None => Value::Null,
        };
    }
}

fn normalize_character(value: &mut Value) {
    {
        let Some(obj) = value.as_object_mut() else {
            return;
        };
        normalize_enum_field(
            obj,
            "action",
            &["FIRST_APPEARANCE", "ACTIVE", "DEPARTURE", "DEATH"],
            "ACTIVE",
            true,
        );
        normalize_opt_string(obj, "role");
        normalize_opt_string(obj, "description");
    }
    normalize_importance(value);
}

fn normalize_item(value: &mut Value) {
    {
        let Some(obj) = value.as_object_mut() else {
            return;
        };
        normalize_enum_field(obj, "category", &["ITEM", "SKILL", "LEGACY"], "ITEM", true);
        normalize_opt_string(obj, "usage_in_chapter");
        normalize_bool(obj, "future_hook_flag");
        if let Some(acquired) = obj.get_mut("acquired_from") {
            if let Some(acquired) = acquired.as_object_mut() {
                normalize_opt_string(acquired, "spatial");
                normalize_opt_string(acquired, "temporal");
            } else {
                *acquired = Value::Null;
            }
        }
    }
    normalize_importance(value);
}

fn normalize_temporal(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    normalize_enum_field(
        obj,
        "type",
        &["CONTINUOUS", "JUMP_FORWARD", "FLASHBACK", "UNCERTAIN"],
        "UNCERTAIN",
        true,
    );
    normalize_opt_string(obj, "raw_marker");
    normalize_opt_string(obj, "reference_anchor");
    if let Some(slot) = obj.get_mut("relative_delta_days") {
        let parsed = match slot {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        };
        *slot = parsed.map(Value::from).unwrap_or(Value::Null);
    }
}

fn normalize_spatial(value: &mut Value) {
    {
        let Some(obj) = value.as_object_mut() else {
            return;
        };
        normalize_opt_string(obj, "world");
        normalize_opt_string(obj, "region");
        normalize_opt_string(obj, "site");
        normalize_opt_string(obj, "source_site");
        normalize_enum_field(
            obj,
            "movement_type",
            &[
                "STAY",
                "LOCAL_MOVE",
                "TELEPORT",
                "ASCENSION",
                "DUNGEON_ENTER",
                "DUNGEON_EXIT",
            ],
            "STAY",
            true,
        );
    }
    normalize_importance(value);
}

fn normalize_enum_field(
    obj: &mut serde_json::Map<String, Value>,
    key: &str,
    allowed: &[&str],
    default: &str,
    uppercase: bool,
) {
    let normalized = obj
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .and_then(|value| {
            let candidate = if uppercase {
                value.to_uppercase()
            } else {
                value.to_lowercase()
            };
            allowed
                .iter()
                .find(|allowed| **allowed == candidate)
                .map(|allowed| (*allowed).to_string())
        })
        .unwrap_or_else(|| default.to_string());
    obj.insert(key.to_string(), Value::String(normalized));
}

fn normalize_opt_string(obj: &mut serde_json::Map<String, Value>, key: &str) {
    let Some(slot) = obj.get_mut(key) else {
        return;
    };
    let replacement = match &*slot {
        Value::Null => Value::Null,
        Value::String(text) => Value::String(text.clone()),
        Value::Number(number) => Value::String(number.to_string()),
        Value::Bool(flag) => Value::String(flag.to_string()),
        _ => Value::String(String::new()),
    };
    *slot = replacement;
}

fn normalize_bool(obj: &mut serde_json::Map<String, Value>, key: &str) {
    let value = match obj.get(key) {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes"
        ),
        Some(Value::Number(number)) => number.as_i64().unwrap_or(0) != 0,
        _ => false,
    };
    obj.insert(key.to_string(), Value::Bool(value));
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
        chapter_index: value
            .get("chapter_index")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        chapter_title: get_str("chapter_title").unwrap_or_default(),
        narrative_perspective: get_str("narrative_perspective"),
        temporal,
        spatial,
        characters: value
            .get("characters")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value::<CharacterEntry>(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default(),
        items_and_skills: value
            .get("items_and_skills")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value::<ItemSkillEntry>(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default(),
        plot_summary: get_str("plot_summary").unwrap_or_default(),
    }
}

/// chunk_chars == 0 表示整章输入：不做切块，直接将整章文本交给模型。
fn chapter_chunks(text: &str, chunk_chars: usize) -> Vec<String> {
    if chunk_chars == 0 {
        vec![text.to_string()]
    } else {
        split_chunks(text, chunk_chars)
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
        if current.chars().count() + para.chars().count() + 1 > target && !current.trim().is_empty()
        {
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

fn build_user_prompt(
    chapter: &ParsedChapter,
    chunk_index: usize,
    chunk_total: usize,
    text: &str,
) -> String {
    format!(
        "【章节序号】{}\n【章节标题】{}\n【分块】{}/{}\n\n【正文】\n{}\n\n【输出要求】\n1. 只分析上面的【正文】，元数据只用于填写 chapter_index 和 chapter_title。\n2. 只输出一个 JSON 对象，不要 Markdown、解释或前后缀文字。\n3. 必须包含全部顶层字段：chapter_index、chapter_title、narrative_perspective、temporal、spatial、characters、items_and_skills、plot_summary。\n4. 严格按系统提示的字段规则和枚举值填写；characters / items_and_skills 没有内容时才用 []。\n5. characters 不要只列主要人物：对话说话人、被称呼者、只被提及者都要检查；全名与简称合并为一条。\n6. 只要正文出现明确场所，spatial.site 就不能为空。\n7. 输出紧凑 JSON（不要缩进美化）；人物/物品很多时压缩 role、description、usage_in_chapter，但必须保留条目、闭合所有括号并完整输出。",
        chapter.chapter_index,
        chapter.title,
        chunk_index + 1,
        chunk_total,
        text
    )
}

fn build_repair_prompt(original: &str, bad_output: &str, error: &str) -> String {
    format!(
        "上一次输出无法解析为合法 JSON。请修复它。\n\n【解析错误】{}\n\n【上一次输出】\n{}\n\n【原始任务】\n{}\n\n【修复要求】\n1. 只修复 JSON 语法和结构，不要改写剧情内容，保留原有字段和值。\n2. 去掉 Markdown 代码块、解释、前言和后语。\n3. 字符串必须用双引号；换行/制表符转义为 \\n / \\t；不要尾随逗号。\n4. 若输出被截断，补全引号和括号，重新输出一个完整对象。\n5. 顶层字段必须齐全：chapter_index、chapter_title、narrative_perspective、temporal、spatial、characters、items_and_skills、plot_summary。\n6. 输入内容很多时压缩 role、description、usage_in_chapter，但不要删除人物/物品条目。\n7. 如果上次输出因长度限制被截断，请重写一个更紧凑的完整 JSON，不要只续写半截对象。\n8. 只输出 JSON，从 {{ 开始、到 }} 结束。",
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

/// 系统提示词：面向小模型，先给字段规则，再给固定模板和输出前检查。
pub const SYSTEM_PROMPT: &str = r#"你是中文小说叙事结构抽取器。任务：读取【正文】，只输出一个合法 JSON 对象。
不要解释、不要思考过程、不要 Markdown、不要代码块、不要“好的/以下是”。第一个字符必须是 {，最后一个字符必须是 }。
本任务适用于任意题材（现实、都市、历史、战争、科幻、武侠、玄幻等），不要预设作品类型。

## 字段规则
- 必须输出全部 8 个顶层字段，不能缺 key，不能输出 null。未知值用 ""、[]、0 或 false。
- chapter_index：原样使用【章节序号】的整数；chapter_title：原样使用【章节标题】。
- narrative_perspective：第一人称写 first_person，第三人称写 third_person。
- temporal.type 只能是 CONTINUOUS（连续）、JUMP_FORWARD（时间后跳）、FLASHBACK（倒叙）、UNCERTAIN（无法判断）之一。raw_marker 抄原文时间词，没有写 ""；relative_delta_days 换算成天数（倒叙用负数，无法判断写 0）；reference_anchor 写所参照的上一事件，没有写 ""。
- spatial.site 必须填写本段人物活动/所在的明确地点（房间、街道、山门、城池、山谷、宫殿等）。只要正文提到一个地名或场所，就必须填 site，不要因为不确定 region/world 而把 site 留空；发生移动时 site 是移动后的所在地，source_site 是起点。
- spatial.world / region：只填原文明确的上级世界/区域，不确定可留空，但不要因此把 site 留空。只有整段完全没有任何地点/场所信息时，site/region/world 才可全空。不要补“现实世界/某城市”等泛化标签，也不要用人物/物品名代替地点名。
- movement_type 只能是 STAY、LOCAL_MOVE、TELEPORT、ASCENSION、DUNGEON_ENTER、DUNGEON_EXIT 之一；没移动写 STAY；仅在明确移动时填 source_site。
- spatial.importance：1–5 的整数，5=全书核心舞台，4=主要地点，3=重要地点，2=次要地点，1=过场地点；不确定写 3。
- characters：列出本段所有具名人物，包括主角、配角、反派、对话的说话人与被称呼者，以及只被提及但未出场者；不要只列剧情摘要中的主要人物。不要列作者、译者、序言作者等现实身份，不要列“众人/士兵/弟子”等群体或“他/她”等代词。同名、绰号、尊称、简称合并为一个规范名，只输出一次；同时出现全名与简称时用全名，简称不要另起条目。name 必须能在正文中找到（第一人称主角找不到名字时写 "PROTAGONIST_FIRST_PERSON"）。
- character.action 只能是 FIRST_APPEARANCE（本段首次出现）、ACTIVE（本段正常出场）、DEPARTURE（明确离开）、DEATH（明确死亡）之一。
- character.role 写具体身份/关系，如“主角”“主角的师父”“反派首领”，不要写“人物/角色/未知”；description 写外貌或身份，没有写 ""。role 不超过 12 字，description 不超过 20 字。
- character.importance：1–5 的整数，5=全书核心主角，4=主要人物，3=重要配角，2=次要人物，1=路人/一次性角色；只被提及一两次的填 1–2。
- items_and_skills：只列本段实际出现或明确被提及的物品/功法/传承；name 用原文写法；category 只能是 ITEM、SKILL、LEGACY 之一。importance 1–5（5=贯穿全书的核心，4=关键，3=重要，2=次要，1=过场）。acquired_from.spatial / temporal 只在原文明确给出时填写，否则写 ""；usage_in_chapter 写本段用途，没有写 ""，不超过 16 字；future_hook_flag 只在原文明确暗示后续用途时写 true，否则 false。
- plot_summary：≤40 字，一句话，包含本段最关键的人物/地点/事件；不要写成人物清单。
- JSON 使用紧凑格式：对象和数组不要换行缩进，不要重复条目。内容很多时压缩描述文字，但必须保留条目并保证 JSON 完整闭合；绝不能中途截断。
- 所有自然语言字段用简体中文；专有名词保留原文写法；仅 JSON 键名与枚举值用英文。
- 严禁编造：只写【正文】中确凿出现的内容；不要套用示例、其它作品或常识。characters / items_and_skills 没有内容时才用 []。

## 数组元素格式
characters 的元素：
{"name":"","action":"ACTIVE","role":"","description":"","importance":3}
items_and_skills 的元素：
{"name":"","category":"ITEM","importance":3,"acquired_from":{"spatial":"","temporal":""},"usage_in_chapter":"","future_hook_flag":false}

## 输出模板（键名、层级、类型必须完全一致）
{
  "chapter_index": 0,
  "chapter_title": "",
  "narrative_perspective": "third_person",
  "temporal": {"type": "CONTINUOUS", "raw_marker": "", "relative_delta_days": 0, "reference_anchor": ""},
  "spatial": {"world": "", "region": "", "site": "", "movement_type": "STAY", "source_site": "", "importance": 3},
  "characters": [],
  "items_and_skills": [],
  "plot_summary": ""
}

## 输出前检查
- 第一个字符是 {，最后一个字符是 }；没有 Markdown、解释或多余文字。
- 8 个顶层字段齐全；字符串用双引号；没有尾随逗号；括号和引号闭合。
- chapter_index / chapter_title 与输入一致；characters / items_and_skills 中的 name 都能在正文找到。
- characters 是否覆盖所有具名人物、对话说话人与被称呼者？同一人物的全名与简称是否已合并？
- 只要正文出现场所，spatial.site 是否已填写？
- 输出是否完整闭合、没有在中途截断？只输出这个 JSON 对象。
"#;

/// 过滤掉未在本段正文中出现的实体，抑制模型幻觉（例如照搬示例或其它作品的人名）。
/// 返回被过滤的条目数量。单字名称跳过校验以避免误删。
fn ground_entries(ext: &mut SegmentExtraction, text: &str) -> usize {
    let normalized_text = normalize_ground(text);
    let mut dropped = 0;
    ext.characters.retain(|c| {
        if c.name == "PROTAGONIST_FIRST_PERSON" || is_grounded(&c.name, text, &normalized_text) {
            true
        } else {
            dropped += 1;
            false
        }
    });
    ext.items_and_skills.retain(|i| {
        if is_grounded(&i.name, text, &normalized_text) {
            true
        } else {
            dropped += 1;
            false
        }
    });
    dropped
}

/// 人名/地名匹配时忽略空格与常见分隔符，避免「久木 祥一郎」被误判为未出现。
fn normalize_ground(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_whitespace()
                && !matches!(
                    c,
                    '·' | '•' | '・' | '，' | ',' | '.' | '。' | '、' | '-' | '_' | '—'
                )
        })
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn is_grounded(name: &str, text: &str, normalized_text: &str) -> bool {
    let n = name.trim();
    if n.chars().count() < 2 {
        return true;
    }
    if text.contains(n) {
        return true;
    }
    let normalized_name = normalize_ground(n);
    if normalized_name.chars().count() < 2 {
        return true;
    }
    normalized_text.contains(&normalized_name)
}

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
        assert_eq!(
            ext.temporal.as_ref().unwrap().relative_delta_days,
            Some(1095)
        );
        assert_eq!(ext.spatial.as_ref().unwrap().world.as_deref(), Some("人界"));
        assert_eq!(ext.characters.len(), 1);
        assert_eq!(ext.characters[0].action, "FIRST_APPEARANCE");
        assert!(ext.items_and_skills[0].future_hook_flag);
    }

    #[test]
    fn parse_extraction_repairs_trailing_comma() {
        let broken = r#"{"chapter_index":1,"characters":[{"name":"韩立"}],"items_and_skills":[],}"#;
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
    fn parse_extraction_coerces_importance() {
        let payload = json!({
            "spatial": {"site": "神手谷", "importance": "4.4"},
            "characters": [
                {"name": "韩立", "importance": "5"},
                {"name": "厉飞雨", "importance": 9}
            ],
            "items_and_skills": [
                {"name": "掌天瓶", "importance": "3.6"},
                {"name": "无名残卷", "importance": -1}
            ]
        })
        .to_string();
        let (ext, _) = parse_extraction(&payload).unwrap();
        assert_eq!(ext.spatial.as_ref().unwrap().importance, Some(4));
        assert_eq!(ext.characters[0].importance, Some(5));
        assert_eq!(ext.characters[1].importance, Some(5));
        assert_eq!(ext.items_and_skills[0].importance, Some(4));
        assert_eq!(ext.items_and_skills[1].importance, Some(1));
    }

    #[test]
    fn parse_extraction_errors_without_json() {
        assert!(parse_extraction("完全没有 JSON 对象").is_err());
    }

    #[test]
    fn extract_json_block_handles_array_and_trailing_text() {
        assert_eq!(
            extract_json_block("结果：[ {\"a\":1}, {\"b\":2} ] 完").unwrap(),
            "{\"a\":1}"
        );
        assert_eq!(
            extract_json_block("{\"temporal\":{\"type\":\"CONTINUOUS\"}}尾").unwrap(),
            "{\"temporal\":{\"type\":\"CONTINUOUS\"}}"
        );
    }

    #[test]
    fn repair_inserts_missing_commas() {
        let broken = "{\"a\": 1\n\"b\": 2}";
        let fixed = repair_json_text(broken);
        let v: Value = serde_json::from_str(&fixed).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn repair_balances_truncated_object() {
        let truncated = "{\"a\": 1, \"characters\": [{\"name\": \"甲\"";
        let fixed = repair_json_text(truncated);
        let v: Value = serde_json::from_str(&fixed).unwrap();
        assert_eq!(v["characters"][0]["name"], "甲");
    }

    #[test]
    fn parse_extraction_repairs_raw_control_chars() {
        // 字符串内直接包含换行/制表符（未转义）——小模型常见错误。
        let broken =
            "{\"plot_summary\":\"第一行\n第二行\t结束\",\"characters\":[],\"items_and_skills\":[]}";
        let (ext, repaired) = parse_extraction(broken).unwrap();
        assert!(repaired);
        assert!(ext.plot_summary.contains("第一行"));
        assert!(ext.plot_summary.contains("第二行"));
    }

    #[test]
    fn ground_entries_drops_hallucinated_names() {
        let mut ext: SegmentExtraction = serde_json::from_value(json!({
            "characters": [
                {"name": "韩立", "action": "ACTIVE"},
                {"name": "张三", "action": "ACTIVE"},
                {"name": "PROTAGONIST_FIRST_PERSON", "action": "ACTIVE"}
            ],
            "items_and_skills": [
                {"name": "掌天瓶", "category": "ITEM"},
                {"name": "不存在的法宝", "category": "ITEM"}
            ]
        }))
        .unwrap();
        let text = "韩立走进屋子，抬眼看了看掌天瓶。";
        let dropped = ground_entries(&mut ext, text);
        assert_eq!(dropped, 2);
        assert_eq!(ext.characters.len(), 2);
        assert!(ext
            .characters
            .iter()
            .any(|c| c.name == "PROTAGONIST_FIRST_PERSON"));
        assert_eq!(ext.items_and_skills.len(), 1);
        assert_eq!(ext.items_and_skills[0].name, "掌天瓶");
    }

    #[test]
    fn lenient_normalization_keeps_people_and_items() {
        let payload = json!({
            "narrative_perspective": "第三人称",
            "characters": [
                {"name": "韩立", "action": "主角", "role": 123, "importance": "5"},
                {"name": "厉飞雨", "action": null, "description": null}
            ],
            "items_and_skills": [
                {"name": "掌天瓶", "category": "法宝", "future_hook_flag": "yes"}
            ]
        })
        .to_string();
        let (ext, _) = parse_extraction(&payload).unwrap();
        assert_eq!(ext.characters.len(), 2, "非法枚举不应丢掉整条人物");
        assert_eq!(ext.characters[0].action, "ACTIVE");
        assert_eq!(ext.characters[0].role.as_deref(), Some("123"));
        assert_eq!(ext.characters[1].action, "ACTIVE");
        assert_eq!(ext.items_and_skills.len(), 1);
        assert_eq!(ext.items_and_skills[0].category, "ITEM");
        assert!(ext.items_and_skills[0].future_hook_flag);
        assert_eq!(ext.narrative_perspective.as_deref(), Some("third_person"));
    }

    #[test]
    fn ground_entries_accepts_names_with_spaces_or_separators() {
        let mut ext: SegmentExtraction = serde_json::from_value(json!({
            "characters": [
                {"name": "久木 祥一郎", "action": "ACTIVE"},
                {"name": "松原·凛子", "action": "ACTIVE"}
            ],
            "items_and_skills": []
        }))
        .unwrap();
        let dropped = ground_entries(&mut ext, "久木祥一郎与松原凛子在轻井泽相遇。");
        assert_eq!(dropped, 0);
        assert_eq!(ext.characters.len(), 2);
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

    #[test]
    fn chapter_chunks_zero_returns_whole_chapter() {
        let text = "第一段\n第二段\n第三段";
        let chunks = chapter_chunks(text, 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn pack_summary_lines_keeps_order_and_budget() {
        let lines = vec![
            "第 1 章：甲".to_string(),
            "第 2 章：乙".to_string(),
            "第 3 章：丙".to_string(),
        ];
        let batches = pack_summary_lines(&lines, 200);
        assert_eq!(batches.len(), 1);
        assert!(batches[0].contains("第 1 章"));
        assert!(batches[0].contains("第 3 章"));

        let long = vec!["甲".repeat(150), "乙".repeat(150), "丙".repeat(150)];
        let batches = pack_summary_lines(&long, 200);
        assert_eq!(batches.len(), 3);
        assert!(batches.iter().all(|b| b.chars().count() <= 200));
    }

    #[test]
    fn parse_summary_output_handles_json_and_plain_text() {
        assert_eq!(
            parse_summary_output("{\"summary\":\"全书总览\"}").as_deref(),
            Some("全书总览")
        );
        assert_eq!(
            parse_summary_output("```json\n{\"summary\":\"带围栏\"}\n```").as_deref(),
            Some("带围栏")
        );
        assert_eq!(
            parse_summary_output("直接输出的总览").as_deref(),
            Some("直接输出的总览")
        );
    }

    #[test]
    fn extraction_prompts_repeat_required_contract() {
        let chapter = ParsedChapter {
            chapter_index: 7,
            title: "第七章".into(),
            text: "正文".into(),
            content_hash: "hash".into(),
            char_count: 2,
        };
        let prompt = build_user_prompt(&chapter, 0, 1, "正文");
        for key in [
            "chapter_index",
            "chapter_title",
            "narrative_perspective",
            "temporal",
            "spatial",
            "characters",
            "items_and_skills",
            "plot_summary",
        ] {
            assert!(prompt.contains(key), "user prompt missing {key}");
            assert!(SYSTEM_PROMPT.contains(key), "system prompt missing {key}");
        }
        assert!(prompt.contains("只输出一个 JSON 对象"));
        assert!(prompt.contains("完整输出"));
        assert!(SYSTEM_PROMPT.contains("输出前检查"));
        assert!(SYSTEM_PROMPT.contains("完整闭合"));
    }

    #[test]
    fn repair_prompt_keeps_literal_json_braces() {
        let prompt = build_repair_prompt("original", "bad output", "expected value");
        assert!(prompt.contains("从 { 开始、到 } 结束"));
        assert!(!prompt.contains("{{"));
    }

    #[test]
    fn summary_validation_rejects_placeholders() {
        assert!(!summary_is_usable("..."));
        assert!(!summary_is_usable("……"));
        assert!(!summary_is_usable("待生成"));
        assert!(!summary_is_usable("{\"summary\":\"...\"}"));
        assert!(summary_is_usable(
            "这是一个真正生成的全书总览，包含主要人物、地点与剧情推进。"
        ));
    }
}
