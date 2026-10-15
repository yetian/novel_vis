use serde::Serialize;

/// 统一的后端错误类型。所有 Tauri 命令都返回 `Result<T, AppError>`，
/// 前端可稳定地解析 `{ code, message }` 结构。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("解析错误: {0}")]
    Parse(String),

    #[error("不支持的文件格式: {0}")]
    UnsupportedFormat(String),

    #[error("网络/LLM 错误: {0}")]
    Llm(String),

    #[error("LLM 返回的 JSON 无法修复: {0}")]
    JsonRepair(String),

    #[error("找不到源文件: {0}")]
    SourceMissing(String),

    #[error("源文件与记录的书籍不一致: {0}")]
    SourceMismatch(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("内部错误: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Database(_) => "DB_ERROR",
            AppError::Parse(_) => "PARSE_ERROR",
            AppError::UnsupportedFormat(_) => "UNSUPPORTED_FORMAT",
            AppError::Llm(_) => "LLM_ERROR",
            AppError::JsonRepair(_) => "JSON_REPAIR_ERROR",
            AppError::SourceMissing(_) => "SOURCE_MISSING",
            AppError::SourceMismatch(_) => "SOURCE_MISMATCH",
            AppError::Io(_) => "IO_ERROR",
            AppError::Serde(_) => "SERDE_ERROR",
            AppError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        AppError::Internal(value.to_string())
    }
}

#[derive(Serialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ErrorPayload {
            code: self.code().to_string(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;
