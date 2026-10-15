use crate::db::Db;
use crate::models::ParsedBook;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 抽取任务的控制句柄，支持暂停 / 继续 / 取消。
pub struct JobControl {
    paused: AtomicBool,
    cancelled: AtomicBool,
}

impl JobControl {
    pub fn new() -> Self {
        JobControl {
            paused: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
        }
    }
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Default for JobControl {
    fn default() -> Self {
        Self::new()
    }
}

/// 应用全局状态。
/// - `db`：SQLite 持久层
/// - `books`：内存中的电子书缓存（原文不落盘，避免磁盘膨胀）
/// - `jobs`：正在运行的抽取任务控制句柄
pub struct AppState {
    pub db: Arc<Db>,
    pub books: Mutex<HashMap<i64, Arc<ParsedBook>>>,
    pub jobs: Mutex<HashMap<i64, Arc<JobControl>>>,
    pub llm: Mutex<Option<crate::models::LlmConfig>>,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        AppState {
            db: Arc::new(db),
            books: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
            llm: Mutex::new(None),
        }
    }

    pub fn register_job(&self, book_id: i64) -> Arc<JobControl> {
        let control = Arc::new(JobControl::new());
        self.jobs.lock().insert(book_id, control.clone());
        control
    }

    pub fn job(&self, book_id: i64) -> Option<Arc<JobControl>> {
        self.jobs.lock().get(&book_id).cloned()
    }

    pub fn clear_job(&self, book_id: i64) {
        self.jobs.lock().remove(&book_id);
    }

    pub fn cache_book(&self, book_id: i64, book: ParsedBook) {
        self.books.lock().insert(book_id, Arc::new(book));
    }

    pub fn book(&self, book_id: i64) -> Option<Arc<ParsedBook>> {
        self.books.lock().get(&book_id).cloned()
    }
}
