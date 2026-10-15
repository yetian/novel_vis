use crate::error::{AppError, AppResult};
use crate::llm;
use crate::models::*;
use crate::parser;
use crate::pipeline;
use crate::state::AppState;
use tauri::{AppHandle, Manager, State};

const LLM_CONFIG_FILE: &str = "llm_config.json";

/// 导入电子书：读取文件到内存，解析分章，写入数据库并缓存原文。
#[tauri::command]
pub async fn ingest_book(state: State<'_, AppState>, path: String) -> AppResult<IngestResult> {
    let bytes = tokio::fs::read(&path).await?;
    let book = parser::parse_book(&path, &bytes)?;

    let db = &state.db;
    let book_id = db.insert_book(
        &book.title,
        book.author.as_deref(),
        &book.format,
        book.chapters.len() as i64,
    )?;
    for ch in &book.chapters {
        db.insert_chapter(book_id, ch.chapter_index, &ch.title, &ch.content_hash, None)?;
    }

    let result = IngestResult {
        book_id,
        title: book.title.clone(),
        author: book.author.clone(),
        format: book.format.clone(),
        total_chapters: book.chapters.len(),
    };
    state.cache_book(book_id, book);
    Ok(result)
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

/// 保存 LLM 引擎配置（LM Studio / Ollama / 自定义）到状态与磁盘。
#[tauri::command]
pub fn save_llm_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: LlmConfig,
) -> AppResult<()> {
    if let Ok(dir) = app.path().app_data_dir() {
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(LLM_CONFIG_FILE);
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            std::fs::write(path, json).ok();
        }
    }
    *state.llm.lock() = Some(config);
    Ok(())
}

#[tauri::command]
pub fn get_llm_config(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<LlmConfig>> {
    if let Some(cfg) = state.llm.lock().clone() {
        return Ok(Some(cfg));
    }
    if let Ok(dir) = app.path().app_data_dir() {
        let path = dir.join(LLM_CONFIG_FILE);
        if let Ok(raw) = std::fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str::<LlmConfig>(&raw) {
                *state.llm.lock() = Some(cfg.clone());
                return Ok(Some(cfg));
            }
        }
    }
    Ok(None)
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

/// 启动批量抽取。
#[tauri::command]
pub async fn start_extraction(
    app: AppHandle,
    state: State<'_, AppState>,
    request: pipeline::ExtractionRequest,
) -> AppResult<Vec<ChapterExtractionResult>> {
    pipeline::run_extraction(app, &state, request).await
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
