pub mod epub;
pub mod mobi;
pub mod util;

use crate::error::{AppError, AppResult};
use crate::models::ParsedBook;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookFormat {
    Epub,
    Mobi,
    Azw3,
    Unknown,
}

impl BookFormat {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            BookFormat::Epub => "EPUB",
            BookFormat::Mobi => "MOBI",
            BookFormat::Azw3 => "AZW3",
            BookFormat::Unknown => "UNKNOWN",
        }
    }
}

/// 结合扩展名与魔数进行格式判定，避免仅依赖文件后缀。
pub fn detect_format(filename: &str, bytes: &[u8]) -> BookFormat {
    let lower = filename.to_lowercase();
    if lower.ends_with(".epub") {
        return BookFormat::Epub;
    }
    if lower.ends_with(".azw3") {
        return BookFormat::Azw3;
    }
    if lower.ends_with(".mobi") {
        return BookFormat::Mobi;
    }
    if bytes.len() > 4 && &bytes[0..4] == b"PK\x03\x04" {
        return BookFormat::Epub;
    }
    if bytes.len() > 68 && &bytes[60..68] == b"BOOKMOBI" {
        return BookFormat::Mobi;
    }
    BookFormat::Unknown
}

/// 在内存中解析电子书。入口统一为 `(filename, bytes)`。
pub fn parse_book(filename: &str, bytes: &[u8]) -> AppResult<ParsedBook> {
    let fallback_title = filename
        .rsplit('/')
        .next()
        .unwrap_or(filename)
        .rsplit_once('.')
        .map(|(n, _)| n.to_string())
        .unwrap_or_else(|| filename.to_string());

    match detect_format(filename, bytes) {
        BookFormat::Epub => epub::parse(bytes, &fallback_title),
        BookFormat::Mobi | BookFormat::Azw3 => mobi::parse(bytes, &fallback_title),
        BookFormat::Unknown => Err(AppError::UnsupportedFormat(format!(
            "无法识别文件格式: {filename} (仅支持 EPUB / MOBI / AZW3)"
        ))),
    }
}
