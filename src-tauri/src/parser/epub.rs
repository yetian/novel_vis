use super::util::{guess_title, hash_text, strip_html};
use crate::error::{AppError, AppResult};
use crate::models::{ParsedBook, ParsedChapter};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::{Cursor, Read};
use zip::ZipArchive;

struct ManifestItem {
    href: String,
}

/// 在内存中解析 EPUB：读取 container.xml -> OPF -> spine，逐章抽取文本。
/// 全部过程基于 `Cursor<Vec<u8>>`，不产生任何临时文件。
pub fn parse(bytes: &[u8], fallback_title: &str) -> AppResult<ParsedBook> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| AppError::Parse(format!("EPUB zip 打开失败: {e}")))?;

    let container = read_zip_entry(&mut archive, "META-INF/container.xml")
        .ok_or_else(|| AppError::Parse("缺少 META-INF/container.xml".into()))?;
    let opf_path = find_rootfile(&container)
        .ok_or_else(|| AppError::Parse("无法定位 OPF 清单文件".into()))?;
    let opf_dir = opf_path
        .rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();

    let opf_bytes = read_zip_entry(&mut archive, &opf_path)
        .ok_or_else(|| AppError::Parse(format!("无法读取 OPF: {opf_path}")))?;
    let opf = parse_opf(&opf_bytes);

    let title = opf.title.clone().unwrap_or_else(|| fallback_title.to_string());

    let mut chapters = Vec::new();
    let mut idx: i64 = 0;
    for idref in &opf.spine {
        let Some(item) = opf.manifest.get(idref) else {
            continue;
        };
        let full = resolve_path(&opf_dir, &item.href);
        let Some(raw) = read_zip_entry(&mut archive, &full) else {
            continue;
        };
        let xhtml = String::from_utf8_lossy(&raw).to_string();
        let text = strip_html(&xhtml);
        if text.chars().count() < 20 {
            continue;
        }
        let chapter_title = extract_html_title(&xhtml).unwrap_or_else(|| guess_title(&text, idx));
        chapters.push(ParsedChapter {
            chapter_index: idx,
            title: chapter_title,
            content_hash: hash_text(&text),
            char_count: text.chars().count(),
            text,
        });
        idx += 1;
    }

    if chapters.is_empty() {
        return Err(AppError::Parse("EPUB 中未解析到任何正文".into()));
    }

    Ok(ParsedBook {
        title,
        author: opf.author,
        format: "EPUB".into(),
        chapters,
    })
}

fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Option<Vec<u8>> {
    let normalized = name.trim_start_matches("./").replace('\\', "/");
    let mut file = archive.by_name(&normalized).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

struct Opf {
    title: Option<String>,
    author: Option<String>,
    manifest: std::collections::HashMap<String, ManifestItem>,
    spine: Vec<String>,
}

fn parse_opf(bytes: &[u8]) -> Opf {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut opf = Opf {
        title: None,
        author: None,
        manifest: std::collections::HashMap::new(),
        spine: Vec::new(),
    };

    let mut in_metadata = false;
    let mut current_text = String::new();
    let mut current_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = local_name(e.name().as_ref());
                if local == "metadata" {
                    in_metadata = true;
                }
                if local == "item" {
                    let mut id = None;
                    let mut href = None;
                    for attr in e.attributes().flatten() {
                        let key = local_name(attr.key.as_ref());
                        let val = attr.unescape_value().unwrap_or_default().into_owned();
                        match key.as_str() {
                            "id" => id = Some(val),
                            "href" => href = Some(val),
                            _ => {}
                        }
                    }
                    if let (Some(id), Some(href)) = (id, href) {
                        opf.manifest.insert(id, ManifestItem { href });
                    }
                }
                if local == "itemref" {
                    for attr in e.attributes().flatten() {
                        if local_name(attr.key.as_ref()) == "idref" {
                            opf.spine
                                .push(attr.unescape_value().unwrap_or_default().into_owned());
                        }
                    }
                }
                current_tag = local;
                current_text.clear();
            }
            Ok(Event::Text(t)) => {
                if in_metadata {
                    current_text.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => {
                let local = local_name(e.name().as_ref());
                if local == "metadata" {
                    in_metadata = false;
                }
                if in_metadata {
                    let value = current_text.trim().to_string();
                    if !value.is_empty() {
                        match local.as_str() {
                            "title" => opf.title = Some(value),
                            "creator" => opf.author = Some(value),
                            _ => {}
                        }
                    }
                }
                current_text.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    let _ = current_tag;
    opf
}

fn find_rootfile(container_xml: &[u8]) -> Option<String> {
    let mut reader = Reader::from_reader(container_xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "rootfile" {
                    for attr in e.attributes().flatten() {
                        if local_name(attr.key.as_ref()) == "full-path" {
                            return Some(attr.unescape_value().unwrap_or_default().into_owned());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

fn extract_html_title(xhtml: &str) -> Option<String> {
    for tag in ["<title>", "<h1", "<h2", "<h3"] {
        if let Some(start) = xhtml.find(tag) {
            let after = &xhtml[start..];
            if let Some(close) = after.find('>') {
                let rest = &after[close + 1..];
                if let Some(end) = rest[..rest.len().min(400)].find('<') {
                    let candidate = strip_html(&rest[..end]);
                    let trimmed = candidate.trim();
                    if !trimmed.is_empty() && trimmed.chars().count() <= 60 {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }
    None
}

fn local_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    match s.rsplit(':').next() {
        Some(local) => local.to_string(),
        None => s.to_string(),
    }
}

fn resolve_path(dir: &str, href: &str) -> String {
    if dir.is_empty() {
        return href.to_string();
    }
    normalize_path(&format!("{dir}/{href}"))
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}
