use crate::error::{AppError, AppResult};
use crate::llm;
use crate::models::*;
use crate::parser;
use crate::paths;
use crate::pipeline;
use crate::state::AppState;
use tauri::{AppHandle, State};

/// 导入前探查：解析文件并判断是否疑似为「一文件多本书」的合订本。
#[tauri::command]
pub async fn inspect_book(path: String) -> AppResult<BookInspection> {
    let bytes = tokio::fs::read(&path).await?;
    let book = parser::parse_book(&path, &bytes)?;

    let suspected = book.candidates.len() >= 2;
    let mut signals = Vec::new();
    if suspected {
        signals.push(format!("目录识别出 {} 个顶级分组", book.candidates.len()));
    }

    Ok(BookInspection {
        title: book.title,
        author: book.author,
        format: book.format,
        total_chapters: book.chapters.len(),
        suspected_multi_book: suspected,
        signals,
        candidates: book.candidates,
    })
}

/// 导入电子书：读取文件到内存，解析分章，写入数据库并缓存原文。
/// `split_into_books` 为真且探测到多个候选书目时，拆分为多本分别入库。
#[tauri::command]
pub async fn ingest_book(
    state: State<'_, AppState>,
    path: String,
    split_into_books: Option<bool>,
) -> AppResult<Vec<IngestResult>> {
    let bytes = tokio::fs::read(&path).await?;
    let book = parser::parse_book(&path, &bytes)?;

    let parts = if split_into_books.unwrap_or(false) && book.candidates.len() >= 2 {
        parser::split_book(&book, &book.candidates)
    } else {
        vec![book]
    };

    let content_sig = parser::util::hash_bytes(&bytes);

    let mut results = Vec::with_capacity(parts.len());
    for part in parts {
        let db = &state.db;
        let book_id = db.insert_book(
            &part.title,
            part.author.as_deref(),
            &part.format,
            part.chapters.len() as i64,
            Some(path.as_str()),
            Some(content_sig.as_str()),
        )?;
        for ch in &part.chapters {
            db.insert_chapter(book_id, ch.chapter_index, &ch.title, &ch.content_hash, None)?;
        }
        results.push(IngestResult {
            book_id,
            title: part.title.clone(),
            author: part.author.clone(),
            format: part.format.clone(),
            total_chapters: part.chapters.len(),
        });
        state.cache_book(book_id, part);
    }

    Ok(results)
}

#[tauri::command]
pub fn list_books(state: State<'_, AppState>) -> AppResult<Vec<BookRow>> {
    state.db.list_books()
}

#[tauri::command]
pub fn get_timeline(state: State<'_, AppState>, book_id: i64) -> AppResult<TimelineBundle> {
    state.db.timeline_bundle(book_id)
}

#[tauri::command]
pub fn get_books_timeline(state: State<'_, AppState>, book_id: i64) -> AppResult<TimelineBundle> {
    state.db.timeline_bundle(book_id)
}

#[tauri::command]
pub fn delete_book(state: State<'_, AppState>, book_id: i64) -> AppResult<()> {
    state.db.delete_book(book_id)
}

/// 清空全部书籍与抽取数据（含内存缓存），用于清除被污染的数据。
#[tauri::command]
pub fn clear_all_data(state: State<'_, AppState>) -> AppResult<()> {
    state.db.delete_all()?;
    state.books.lock().clear();
    Ok(())
}

/// 保存 LLM 引擎配置（LM Studio / Ollama / 自定义）到状态与磁盘。
#[tauri::command]
pub fn save_llm_config(state: State<'_, AppState>, config: LlmConfig) -> AppResult<()> {
    if paths::ensure_data_dir().is_ok() {
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            std::fs::write(paths::llm_config_path(), json).ok();
        }
    }
    *state.llm.lock() = Some(config);
    Ok(())
}

#[tauri::command]
pub fn get_llm_config(state: State<'_, AppState>) -> AppResult<Option<LlmConfig>> {
    if let Some(cfg) = state.llm.lock().clone() {
        return Ok(Some(cfg));
    }
    if let Ok(raw) = std::fs::read_to_string(paths::llm_config_path()) {
        if let Ok(cfg) = serde_json::from_str::<LlmConfig>(&raw) {
            *state.llm.lock() = Some(cfg.clone());
            return Ok(Some(cfg));
        }
    }
    Ok(None)
}

/// 返回应用数据目录（供界面展示与排查）。
#[tauri::command]
pub fn get_data_dir() -> AppResult<String> {
    Ok(paths::data_dir().to_string_lossy().to_string())
}

/// 通过 OpenAI 兼容 `/v1/models`（或 Ollama `/api/tags`）自动探测模型列表。
#[tauri::command]
pub async fn list_models(config: LlmConfig) -> AppResult<Vec<ModelInfo>> {
    llm::list_models(&config).await
}

#[tauri::command]
pub async fn test_connection(config: LlmConfig) -> AppResult<bool> {
    llm::health_check(&config).await
}

/// 确保书籍原文已加载到内存：优先命中缓存，否则按记录的路径重新读取并校验签名。
fn ensure_book_loaded(state: &AppState, book_id: i64) -> AppResult<()> {
    if state.book(book_id).is_some() {
        return Ok(());
    }
    let (path, sig) = state.db.book_source(book_id)?;
    let path = path.ok_or_else(|| {
        AppError::SourceMissing(format!("书籍 #{book_id} 未记录源文件路径，请重新导入"))
    })?;
    let bytes = std::fs::read(&path)
        .map_err(|e| AppError::SourceMissing(format!("无法读取 {path}：{e}")))?;
    if let Some(expected) = sig {
        if parser::util::hash_bytes(&bytes) != expected {
            return Err(AppError::SourceMismatch(path));
        }
    }
    let parsed = parser::parse_book(&path, &bytes)?;
    state.cache_book(book_id, parsed);
    Ok(())
}

/// 启动批量抽取。若原文不在内存中，会尝试按记录的源文件路径重新加载。
#[tauri::command]
pub async fn start_extraction(
    app: AppHandle,
    state: State<'_, AppState>,
    request: pipeline::ExtractionRequest,
) -> AppResult<Vec<ChapterExtractionResult>> {
    ensure_book_loaded(&state, request.book_id)?;
    pipeline::run_extraction(app, &state, request).await
}

/// 重新定位并关联书籍的源文件。`force` 为真时允许关联内容不一致的文件。
#[tauri::command]
pub async fn set_book_source(
    state: State<'_, AppState>,
    book_id: i64,
    path: String,
    force: Option<bool>,
) -> AppResult<SourceCheck> {
    let bytes = tokio::fs::read(&path).await?;
    let parsed = parser::parse_book(&path, &bytes)?;
    let new_sig = parser::util::hash_bytes(&bytes);
    let (_, old_sig) = state.db.book_source(book_id)?;
    let matched = old_sig.as_deref().map(|s| s == new_sig).unwrap_or(true);

    if !matched && !force.unwrap_or(false) {
        return Ok(SourceCheck {
            matched: false,
            message: format!("所选文件与记录的书籍内容不一致：{path}"),
        });
    }

    state.db.update_book_source(book_id, Some(&path), Some(&new_sig))?;
    state.cache_book(book_id, parsed);
    Ok(SourceCheck {
        matched,
        message: if matched {
            "已重新关联源文件".into()
        } else {
            "已强制关联到新的源文件".into()
        },
    })
}

#[tauri::command]
pub fn get_book_source(state: State<'_, AppState>, book_id: i64) -> AppResult<Option<String>> {
    Ok(state.db.book_source(book_id)?.0)
}

#[tauri::command]
pub fn pause_extraction(state: State<'_, AppState>, book_id: i64) -> AppResult<()> {
    if let Some(job) = state.job(book_id) {
        job.pause();
    }
    Ok(())
}

#[tauri::command]
pub fn resume_extraction(state: State<'_, AppState>, book_id: i64) -> AppResult<()> {
    if let Some(job) = state.job(book_id) {
        job.resume();
    }
    Ok(())
}

#[tauri::command]
pub fn cancel_extraction(state: State<'_, AppState>, book_id: i64) -> AppResult<()> {
    if let Some(job) = state.job(book_id) {
        job.cancel();
    }
    Ok(())
}

/// 读取文件为文本（供前端静态 HTML 导出时使用）。
#[tauri::command]
pub async fn read_text_file(path: String) -> AppResult<String> {
    Ok(tokio::fs::read_to_string(path).await.map_err(AppError::from)?)
}
