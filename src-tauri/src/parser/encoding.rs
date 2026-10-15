//! 中文友好的文本解码。
//!
//! 中文电子书编码复杂：EPUB 多为 UTF-8（但偶有 GB18030），
//! 而 MOBI/AZW3 常见 GBK/Big5，甚至把 GBK 字节错误标记为 windows-1252。
//! 因此不能盲信声明的编码，而应在候选编码中挑选“中文可读性”最高者。

use encoding_rs::{Encoding, GBK, UTF_8, WINDOWS_1252};

/// 在声明的编码不可靠时，参与打分的候选编码（按优先级排列）。
const CANDIDATES: [&'static Encoding; 3] = [UTF_8, GBK, WINDOWS_1252];

/// 智能解码：优先 BOM → 可靠的 Unicode 声明 → 候选打分。
pub fn smart_decode(bytes: &[u8], declared: Option<&str>) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    // 1) BOM 最可靠
    if let Some((enc, _)) = Encoding::for_bom(bytes) {
        let (cow, _, _) = enc.decode(bytes);
        return cow.into_owned();
    }

    // 2) 仅信任 UTF-8 / UTF-16 这类可靠声明
    if let Some(name) = declared {
        if let Some(enc) = Encoding::for_label(name.trim().as_bytes()) {
            if is_reliable_unicode(enc) {
                let (cow, _, had_errors) = enc.decode(bytes);
                if !had_errors {
                    return cow.into_owned();
                }
            }
        }
    }

    // 3) 候选打分（GBK 字节被误标为 1252 时也能纠正为中文）
    let mut best: Option<(isize, String)> = None;
    for enc in CANDIDATES {
        let (cow, _, _) = enc.decode(bytes);
        let decoded = cow.into_owned();
        let score = readability_score(&decoded);
        if best.as_ref().map(|(b, _)| score > *b).unwrap_or(true) {
            best = Some((score, decoded));
        }
    }

    match best {
        Some((_, s)) => s,
        None => String::from_utf8_lossy(bytes).to_string(),
    }
}

fn is_reliable_unicode(enc: &'static Encoding) -> bool {
    matches!(enc.name(), "UTF-8" | "UTF-16LE" | "UTF-16BE")
}

/// 中文可读性评分：CJK 字符加分，替换符/控制符扣分。
fn readability_score(s: &str) -> isize {
    let mut score: isize = 0;
    for c in s.chars() {
        let cp = c as u32;
        let is_cjk = (0x4E00..=0x9FFF).contains(&cp)      // 基本汉字
            || (0x3400..=0x4DBF).contains(&cp)             // 扩展 A
            || (0x3000..=0x303F).contains(&cp)             // CJK 标点
            || (0xFF00..=0xFFEF).contains(&cp); // 全角字符
        if is_cjk {
            score += 2;
        } else if c == '\u{FFFD}' {
            score -= 5;
        } else if cp < 0x20 && c != '\n' && c != '\r' && c != '\t' {
            score -= 3;
        }
    }
    score
}

/// 从 XML/HTML 声明中嗅探编码名，例如 `<?xml version="1.0" encoding="GB18030"?>`。
pub fn sniff_xml_encoding(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(256)];
    let text = String::from_utf8_lossy(head);
    let lower = text.to_lowercase();
    let idx = lower.find("encoding")?;
    let after = &text[idx + "encoding".len()..];
    let eq = after.find('=')?;
    let rest = after[eq + 1..].trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &rest[quote.len_utf8()..];
    let end = rest.find(quote)?;
    let name = rest[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::GBK;

    #[test]
    fn decodes_plain_utf8() {
        let text = "时空编年史 · 韩立拾得掌天瓶";
        assert_eq!(smart_decode(text.as_bytes(), None), text);
    }

    #[test]
    fn decodes_gbk_bytes() {
        let text = "凡人修仙传：韩立于神手谷拾得掌天瓶。";
        let (bytes, _, _) = GBK.encode(text);
        assert_eq!(smart_decode(&bytes, Some("gbk")), text);
        // 未声明时也应能启发式还原
        assert_eq!(smart_decode(&bytes, None), text);
    }

    #[test]
    fn corrects_gbk_mislabeled_as_windows_1252() {
        let text = "越国七玄门，神手谷后山。";
        let (bytes, _, _) = GBK.encode(text);
        // 中文 MOBI 常见的错误标记：textEncoding=1252
        assert_eq!(smart_decode(&bytes, Some("windows-1252")), text);
    }

    #[test]
    fn handles_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("楔子".as_bytes());
        assert_eq!(smart_decode(&bytes, None), "楔子");
    }

    #[test]
    fn sniffs_xml_declared_encoding() {
        let xml = br#"<?xml version="1.0" encoding="GB18030"?><html/>"#;
        assert_eq!(sniff_xml_encoding(xml).as_deref(), Some("GB18030"));
        let xml2 = b"<?xml version='1.0' encoding='Big5'?>";
        assert_eq!(sniff_xml_encoding(xml2).as_deref(), Some("Big5"));
        assert_eq!(sniff_xml_encoding(b"<html></html>"), None);
    }
}
