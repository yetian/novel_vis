use super::util::{guess_title, hash_text, strip_html};
use crate::error::{AppError, AppResult};
use crate::models::{ParsedBook, ParsedChapter};

const PALMDB_HEADER_LEN: usize = 78;
const RECORD_INFO_LEN: usize = 8;

/// PalmDOC 压缩类型
const COMPRESSION_NONE: u16 = 1;
const COMPRESSION_PALMDOC: u16 = 2;
const COMPRESSION_HUFFCDIC: u16 = 17480;

struct Pdb {
    records: Vec<(usize, usize)>, // (offset, length)
    title: String,
}

/// 在内存中解析 MOBI / AZW3 (KF8)。AZW3 本质为 MOBI 容器，
/// 文本部分复用相同的 PalmDOC 记录结构。
pub fn parse(bytes: &[u8], fallback_title: &str) -> AppResult<ParsedBook> {
    let pdb = parse_pdb(bytes)?;
    let record0 = read_record(bytes, &pdb, 0)?;

    if record0.len() < 16 {
        return Err(AppError::Parse("MOBI 头记录长度不足".into()));
    }
    let compression = u16::from_be_bytes([record0[0], record0[1]]);
    let text_length = u32::from_be_bytes([record0[4], record0[5], record0[6], record0[7]]) as usize;
    let text_record_count = u16::from_be_bytes([record0[8], record0[9]]) as usize;
    let encryption = u16::from_be_bytes([record0[12], record0[13]]);

    if encryption != 0 {
        return Err(AppError::UnsupportedFormat(
            "该 MOBI/AZW3 受 DRM 加密，暂不支持解析".into(),
        ));
    }

    let encoding = detect_encoding(&record0);
    let title = extract_exth_title(&record0)
        .filter(|t| !t.is_empty())
        .or_else(|| Some(pdb.title.clone()).filter(|t| !t.is_empty()))
        .unwrap_or_else(|| fallback_title.to_string());

    let mut raw: Vec<u8> = Vec::with_capacity(text_length.max(1024));
    for i in 1..=text_record_count {
        let rec = read_record(bytes, &pdb, i)?;
        let decompressed = match compression {
            COMPRESSION_NONE => rec.to_vec(),
            COMPRESSION_PALMDOC => palmdoc_decompress(rec),
            COMPRESSION_HUFFCDIC => {
                return Err(AppError::UnsupportedFormat(
                    "HUFF/CDIC 压缩的 MOBI 暂不支持".into(),
                ))
            }
            other => {
                return Err(AppError::UnsupportedFormat(format!(
                    "未知的 MOBI 压缩类型: {other}"
                )))
            }
        };
        raw.extend_from_slice(&decompressed);
        if raw.len() >= text_length {
            break;
        }
    }
    raw.truncate(text_length);

    let decoded = super::encoding::smart_decode(&raw, declared_name(encoding));
    let html = decoded.replace("mbp:pagebreak", "\n@@PAGEBREAK@@\n");
    let plain = strip_html(&html);

    let segments: Vec<String> = plain
        .split("@@PAGEBREAK@@")
        .map(|s| s.trim().to_string())
        .filter(|s| s.chars().count() > 20)
        .collect();

    let segments = if segments.len() <= 1 {
        chunk_by_size(&plain, 12_000)
    } else {
        segments
    };

    let mut chapters = Vec::new();
    for (i, text) in segments.into_iter().enumerate() {
        let idx = i as i64;
        let chapter_title = guess_title(&text, idx);
        chapters.push(ParsedChapter {
            chapter_index: idx,
            title: chapter_title,
            content_hash: hash_text(&text),
            char_count: text.chars().count(),
            text,
        });
    }

    if chapters.is_empty() {
        return Err(AppError::Parse("MOBI 中未解析到正文".into()));
    }

    let format = if pdb.title.to_lowercase().contains("azw") {
        "AZW3"
    } else {
        "MOBI"
    };

    Ok(ParsedBook {
        title,
        author: None,
        format: format.into(),
        chapters,
        candidates: Vec::new(),
    })
}

fn parse_pdb(bytes: &[u8]) -> AppResult<Pdb> {
    if bytes.len() < PALMDB_HEADER_LEN {
        return Err(AppError::Parse("文件过小，不是有效的 MOBI".into()));
    }
    let num_records = u16::from_be_bytes([bytes[76], bytes[77]]) as usize;
    let record_info_start = PALMDB_HEADER_LEN;
    let record_info_end = record_info_start + num_records * RECORD_INFO_LEN;
    if bytes.len() < record_info_end {
        return Err(AppError::Parse("记录索引越界".into()));
    }

    let mut records = Vec::with_capacity(num_records);
    for i in 0..num_records {
        let base = record_info_start + i * RECORD_INFO_LEN;
        let offset = u32::from_be_bytes([
            bytes[base],
            bytes[base + 1],
            bytes[base + 2],
            bytes[base + 3],
        ]) as usize;
        records.push(offset);
    }

    let mut ranges = Vec::with_capacity(records.len());
    for i in 0..records.len() {
        let start = records[i];
        let end = if i + 1 < records.len() {
            records[i + 1]
        } else {
            bytes.len()
        };
        if start > bytes.len() || end > bytes.len() || end < start {
            return Err(AppError::Parse("记录偏移非法".into()));
        }
        ranges.push((start, end - start));
    }

    let title = String::from_utf8_lossy(&bytes[0..32])
        .trim_matches(char::from(0))
        .trim()
        .to_string();

    Ok(Pdb {
        records: ranges,
        title,
    })
}

fn read_record<'a>(bytes: &'a [u8], pdb: &Pdb, index: usize) -> AppResult<&'a [u8]> {
    let (offset, len) = pdb
        .records
        .get(index)
        .copied()
        .ok_or_else(|| AppError::Parse(format!("记录 {index} 不存在")))?;
    Ok(&bytes[offset..offset + len])
}

/// PalmDOC (LZ77 变体) 解压。
fn palmdoc_decompress(input: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(input.len() * 4);
    let mut i = 0;
    while i < input.len() {
        let c = input[i];
        i += 1;
        match c {
            0x00 => out.push(0x00),
            0x01..=0x08 => out.push(c),
            0x09..=0x7f => out.push(c),
            0x80..=0xbf => {
                if i >= input.len() {
                    break;
                }
                let c2 = input[i];
                i += 1;
                let pair = ((c as u16) << 8) | c2 as u16;
                let distance = ((pair >> 3) & 0x07ff) as usize;
                let length = (pair & 0x0007) as usize + 3;
                if distance == 0 {
                    continue;
                }
                let start = out.len().saturating_sub(distance);
                for k in 0..length {
                    let b = out.get(start + k).copied().unwrap_or(0);
                    out.push(b);
                }
            }
            0xc0..=0xff => {
                out.push(b' ');
                out.push(c ^ 0x80);
            }
        }
    }
    out
}

fn detect_encoding(record0: &[u8]) -> u32 {
    // MOBI header 位于 record0 偏移 16 处，textEncoding 在 MOBI 内偏移 0x1C。
    if record0.len() >= 16 + 0x20 && &record0[16..20] == b"MOBI" {
        let off = 16 + 0x1C;
        u32::from_be_bytes([record0[off], record0[off + 1], record0[off + 2], record0[off + 3]])
    } else {
        65001
    }
}

/// 将 MOBI 头的 textEncoding 数值映射为编码名。
/// `1252` / 未知值返回 `None`，交由启发式判定（中文书常把 GBK 误标为 1252）。
fn declared_name(code: u32) -> Option<&'static str> {
    match code {
        65001 => Some("utf-8"),
        1200 => Some("utf-16le"),
        1201 => Some("utf-16be"),
        936 => Some("gbk"),
        950 => Some("big5"),
        932 => Some("shift_jis"),
        20936 => Some("gb18030"),
        _ => None,
    }
}

/// 从 EXTH 记录中读取书名 (type 503) 或作者 (type 100)。
fn extract_exth_title(record0: &[u8]) -> Option<String> {
    let mobi_len = u32::from_be_bytes([record0[20], record0[21], record0[22], record0[23]]) as usize;
    let exth_start = 16 + mobi_len;
    if exth_start + 12 > record0.len() || &record0[exth_start..exth_start + 4] != b"EXTH" {
        return None;
    }
    let rec_count =
        u32::from_be_bytes([record0[exth_start + 8], record0[exth_start + 9], record0[exth_start + 10], record0[exth_start + 11]])
            as usize;
    let mut pos = exth_start + 12;
    for _ in 0..rec_count {
        if pos + 8 > record0.len() {
            break;
        }
        let rec_type =
            u32::from_be_bytes([record0[pos], record0[pos + 1], record0[pos + 2], record0[pos + 3]]);
        let rec_len =
            u32::from_be_bytes([record0[pos + 4], record0[pos + 5], record0[pos + 6], record0[pos + 7]])
                as usize;
        if rec_len < 8 || pos + rec_len > record0.len() {
            break;
        }
        if rec_type == 503 {
            let data = &record0[pos + 8..pos + rec_len];
            return Some(String::from_utf8_lossy(data).trim().to_string());
        }
        pos += rec_len;
    }
    None
}

fn chunk_by_size(text: &str, size: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(size)
        .map(|c| c.iter().collect::<String>().trim().to_string())
        .filter(|s| s.chars().count() > 20)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palmdoc_passes_literals() {
        assert_eq!(palmdoc_decompress(b"ABC"), b"ABC".to_vec());
    }

    #[test]
    fn palmdoc_expands_space_char_pairs() {
        // 0xC0..0xFF -> 空格 + (byte ^ 0x80)
        assert_eq!(palmdoc_decompress(&[0xC1]), b" A".to_vec());
        assert_eq!(palmdoc_decompress(&[0xC0]), b" @".to_vec());
    }

    #[test]
    fn palmdoc_resolves_back_reference() {
        // "ABC" 之后 distance=3, length=3 -> "ABCABC"
        let mut input = b"ABC".to_vec();
        input.extend_from_slice(&[0x80, 0x18]);
        assert_eq!(palmdoc_decompress(&input), b"ABCABC".to_vec());
    }
}
