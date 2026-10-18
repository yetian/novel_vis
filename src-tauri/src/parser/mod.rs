pub mod encoding;
pub mod epub;
pub mod mobi;
pub mod util;

use crate::error::{AppError, AppResult};
use crate::models::{BookCandidate, ParsedBook};
use std::collections::BTreeSet;

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

/// 按选中的目录节点把电子书拆分为多本；每个节点包含其区间内的所有后代章节。
/// 未被选中的节点不会导入，也不会额外生成「其他」一书。
pub fn split_book_selected(book: &ParsedBook, candidates: &[BookCandidate]) -> Vec<ParsedBook> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    for candidate in candidates {
        let mut chapters = Vec::new();
        for ch in &book.chapters {
            if ch.chapter_index >= candidate.start_index && ch.chapter_index <= candidate.end_index
            {
                let mut cloned = ch.clone();
                cloned.chapter_index = chapters.len() as i64;
                chapters.push(cloned);
            }
        }
        if chapters.is_empty() {
            continue;
        }
        result.push(ParsedBook {
            title: candidate.title.clone(),
            author: book.author.clone(),
            format: book.format.clone(),
            chapters,
            candidates: Vec::new(),
        });
    }
    result
}

/// 按候选书目把一本电子书拆分为多本。
/// 候选覆盖不到的章节会归入一本「<书名> · 其他」，避免正文丢失。
pub fn split_book(book: &ParsedBook, candidates: &[BookCandidate]) -> Vec<ParsedBook> {
    if candidates.len() < 2 {
        return vec![book.clone()];
    }

    let covered: BTreeSet<i64> = candidates
        .iter()
        .flat_map(|c| c.start_index..=c.end_index)
        .collect();
    let mut result = split_book_selected(book, candidates);

    let leftover: Vec<_> = book
        .chapters
        .iter()
        .filter(|c| !covered.contains(&c.chapter_index))
        .cloned()
        .collect();
    if !leftover.is_empty() {
        let mut chapters = leftover;
        for (i, ch) in chapters.iter_mut().enumerate() {
            ch.chapter_index = i as i64;
        }
        result.push(ParsedBook {
            title: format!("{} · 其他", book.title),
            author: book.author.clone(),
            format: book.format.clone(),
            chapters,
            candidates: Vec::new(),
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_by_extension_case_insensitive() {
        assert_eq!(detect_format("a.EPUB", b""), BookFormat::Epub);
        assert_eq!(detect_format("a.mobi", b""), BookFormat::Mobi);
        assert_eq!(detect_format("a.azw3", b""), BookFormat::Azw3);
    }

    #[test]
    fn detect_by_magic_bytes() {
        let epub = b"PK\x03\x04rest-of-zip".to_vec();
        assert_eq!(detect_format("noext", &epub), BookFormat::Epub);

        let mut mobi = vec![0u8; 72];
        mobi[60..68].copy_from_slice(b"BOOKMOBI");
        assert_eq!(detect_format("noext", &mobi), BookFormat::Mobi);
    }

    #[test]
    fn detect_unknown_falls_through() {
        assert_eq!(detect_format("a.txt", b"plain text"), BookFormat::Unknown);
        assert!(parse_book("a.txt", b"plain text").is_err());
    }

    fn book_with(n: i64) -> crate::models::ParsedBook {
        let chapters = (0..n)
            .map(|i| crate::models::ParsedChapter {
                chapter_index: i,
                title: format!("第 {} 章", i + 1),
                text: "正文".into(),
                content_hash: format!("h{i}"),
                char_count: 2,
            })
            .collect();
        crate::models::ParsedBook {
            title: "合集".into(),
            author: Some("作者".into()),
            format: "EPUB".into(),
            chapters,
            candidates: Vec::new(),
        }
    }

    #[test]
    fn split_book_partitions_and_reindexes() {
        let book = book_with(4);
        let candidates = vec![
            BookCandidate {
                id: "0".into(),
                title: "上卷".into(),
                start_index: 0,
                end_index: 1,
                chapter_count: 2,
                depth: 0,
                children: vec![],
            },
            BookCandidate {
                id: "1".into(),
                title: "下卷".into(),
                start_index: 2,
                end_index: 3,
                chapter_count: 2,
                depth: 0,
                children: vec![],
            },
        ];
        let parts = split_book(&book, &candidates);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].title, "上卷");
        assert_eq!(parts[0].chapters.len(), 2);
        assert_eq!(parts[0].chapters[0].chapter_index, 0);
        assert_eq!(parts[0].chapters[1].chapter_index, 1);
        assert_eq!(parts[1].title, "下卷");
        assert_eq!(parts[1].chapters[0].chapter_index, 0);
    }

    #[test]
    fn split_book_collects_uncovered_chapters() {
        let book = book_with(3);
        let candidates = vec![BookCandidate {
            id: "0".into(),
            title: "上卷".into(),
            start_index: 1,
            end_index: 1,
            chapter_count: 1,
            depth: 0,
            children: vec![],
        }];
        // 少于 2 个候选 -> 不拆分
        assert_eq!(split_book(&book, &candidates).len(), 1);

        let candidates = vec![
            BookCandidate {
                id: "0".into(),
                title: "上卷".into(),
                start_index: 1,
                end_index: 1,
                chapter_count: 1,
                depth: 0,
                children: vec![],
            },
            BookCandidate {
                id: "1".into(),
                title: "下卷".into(),
                start_index: 2,
                end_index: 2,
                chapter_count: 1,
                depth: 0,
                children: vec![],
            },
        ];
        let parts = split_book(&book, &candidates);
        assert_eq!(parts.len(), 3, "第 0 章应归入「其他」");
        assert_eq!(parts[2].title, "合集 · 其他");
        assert_eq!(parts[2].chapters.len(), 1);
        assert_eq!(parts[2].chapters[0].chapter_index, 0);
    }

    #[test]
    fn split_book_selected_includes_range_and_skips_unselected() {
        let book = book_with(4);
        let chosen = vec![BookCandidate {
            id: "0".into(),
            title: "选中的书".into(),
            start_index: 0,
            end_index: 1,
            chapter_count: 2,
            depth: 0,
            children: vec![],
        }];
        let parts = split_book_selected(&book, &chosen);
        assert_eq!(parts.len(), 1, "只导入选中的主章节");
        assert_eq!(parts[0].title, "选中的书");
        assert_eq!(parts[0].chapters.len(), 2, "子章节包含在主章节范围内");
        assert_eq!(parts[0].chapters[0].chapter_index, 0);
        assert_eq!(parts[0].chapters[1].chapter_index, 1);
        assert!(split_book_selected(&book, &[]).is_empty());
    }
}
