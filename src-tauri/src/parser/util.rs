use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};

static SCRIPT_STYLE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<(script|style)[^>]*>.*?</(script|style)>").unwrap());
static BLOCK_BREAK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)</?(p|div|br|h[1-6]|li|tr|section|article|blockquote)[^>]*>").unwrap()
});
static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<[^>]+>").unwrap());
static WS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t\u{00a0}]+").unwrap());
static MULTI_NL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());

/// 将 XHTML/HTML 片段转换为纯文本，保留段落换行。
pub fn strip_html(input: &str) -> String {
    let no_script = SCRIPT_STYLE_RE.replace_all(input, " ");
    let with_breaks = BLOCK_BREAK_RE.replace_all(&no_script, "\n");
    let no_tags = TAG_RE.replace_all(&with_breaks, "");
    let decoded = decode_entities(&no_tags);
    let collapsed = WS_RE.replace_all(&decoded, " ");
    let finalized = MULTI_NL_RE.replace_all(&collapsed, "\n\n");
    finalized
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// 支持常见的 HTML 实体与数字实体。
pub fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '&' {
            if let Some(semi) = bytes[i..].iter().position(|&c| c == ';').map(|p| i + p) {
                if semi - i <= 12 {
                    let entity: String = bytes[i + 1..semi].iter().collect();
                    if let Some(repl) = decode_one(&entity) {
                        out.push_str(&repl);
                        i = semi + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn decode_one(entity: &str) -> Option<String> {
    if let Some(num) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
        return u32::from_str_radix(num, 16).ok().and_then(char::from_u32).map(|c| c.to_string());
    }
    if let Some(num) = entity.strip_prefix('#') {
        return num.parse::<u32>().ok().and_then(char::from_u32).map(|c| c.to_string());
    }
    let s = match entity {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        "mdash" => "—",
        "ndash" => "–",
        "hellip" => "…",
        "ldquo" => "“",
        "rdquo" => "”",
        "lsquo" => "‘",
        "rsquo" => "’",
        _ => return None,
    };
    Some(s.to_string())
}

pub fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 从文本中猜测一个章节标题（用于无结构化目录的格式）。
pub fn guess_title(text: &str, fallback_index: i64) -> String {
    for line in text.lines().take(6) {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() <= 40 {
            let lower = t.to_lowercase();
            if lower.starts_with("chapter")
                || t.starts_with('第')
                || t.starts_with("序")
                || t.starts_with("楔子")
                || t.starts_with("卷")
            {
                return t.to_string();
            }
        }
    }
    format!("第 {} 章", fallback_index + 1)
}
