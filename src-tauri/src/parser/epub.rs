use super::util::{guess_title_with_book, hash_text, strip_html};
use crate::error::{AppError, AppResult};
use crate::models::{BookCandidate, ParsedBook, ParsedChapter};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::{Cursor, Read};
use zip::ZipArchive;

struct ManifestItem {
    href: String,
}

/// 目录导航条目（EPUB3 nav 或 NCX），保留层级以便推断合订本结构。
#[derive(Debug, Default, Clone)]
struct NavEntry {
    title: String,
    href: Option<String>,
    children: Vec<NavEntry>,
}

struct Opf {
    title: Option<String>,
    titles: Vec<String>,
    author: Option<String>,
    manifest: HashMap<String, ManifestItem>,
    spine: Vec<String>,
    nav_href: Option<String>,
    ncx_href: Option<String>,
}

/// 在内存中解析 EPUB：读取 container.xml -> OPF -> spine 逐章抽取文本，
/// 并解析目录（NCX / EPUB3 nav）以探测可能的合订本结构。
pub fn parse(bytes: &[u8], fallback_title: &str) -> AppResult<ParsedBook> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| AppError::Parse(format!("EPUB zip 打开失败: {e}")))?;

    let container = read_zip_entry(&mut archive, "META-INF/container.xml")
        .ok_or_else(|| AppError::Parse("缺少 META-INF/container.xml".into()))?;
    let opf_path =
        find_rootfile(&container).ok_or_else(|| AppError::Parse("无法定位 OPF 清单文件".into()))?;
    let opf_dir = dir_of(&opf_path);

    let opf_bytes = read_zip_entry(&mut archive, &opf_path)
        .ok_or_else(|| AppError::Parse(format!("无法读取 OPF: {opf_path}")))?;
    let opf = parse_opf(&opf_bytes);

    let title = opf
        .titles
        .first()
        .cloned()
        .or_else(|| opf.title.clone())
        .unwrap_or_else(|| fallback_title.to_string());

    let mut chapters = Vec::new();
    let mut path_to_index: HashMap<String, i64> = HashMap::new();
    let mut idx: i64 = 0;
    for idref in &opf.spine {
        let Some(item) = opf.manifest.get(idref) else {
            continue;
        };
        let full = resolve_path(&opf_dir, &item.href);
        let Some(raw) = read_zip_entry(&mut archive, &full) else {
            continue;
        };
        // EPUB 规范要求 UTF-8，但中文书偶有 GB18030/Big5，故按声明 + 启发式解码。
        let declared = super::encoding::sniff_xml_encoding(&raw);
        let xhtml = super::encoding::smart_decode(&raw, declared.as_deref());
        let text = strip_html(&xhtml);
        if text.chars().count() < 20 {
            continue;
        }
        let chapter_title = extract_html_title(&xhtml, Some(&title))
            .unwrap_or_else(|| guess_title_with_book(&text, idx, Some(&title)));
        chapters.push(ParsedChapter {
            chapter_index: idx,
            title: chapter_title,
            content_hash: hash_text(&text),
            char_count: text.chars().count(),
            text,
        });
        path_to_index.insert(full, idx);
        idx += 1;
    }

    if chapters.is_empty() {
        return Err(AppError::Parse("EPUB 中未解析到任何正文".into()));
    }

    // 解析导航（优先 EPUB3 nav，其次 NCX）
    let (nav_entries, nav_base_dir) = read_navigation(&mut archive, &opf, &opf_dir);
    let candidates = build_candidates(&nav_entries, &nav_base_dir, &path_to_index);

    Ok(ParsedBook {
        title,
        author: opf.author,
        format: "EPUB".into(),
        chapters,
        candidates,
    })
}

fn read_navigation<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    opf: &Opf,
    opf_dir: &str,
) -> (Vec<NavEntry>, String) {
    if let Some(nav) = &opf.nav_href {
        let full = resolve_path(opf_dir, nav);
        let base = dir_of(&full);
        let entries = read_zip_entry(archive, &full)
            .map(|b| parse_nav(&b))
            .unwrap_or_default();
        if !entries.is_empty() {
            return (entries, base);
        }
    }
    if let Some(ncx) = &opf.ncx_href {
        let full = resolve_path(opf_dir, ncx);
        let base = dir_of(&full);
        let entries = read_zip_entry(archive, &full)
            .map(|b| parse_ncx(&b))
            .unwrap_or_default();
        return (entries, base);
    }
    (Vec::new(), opf_dir.to_string())
}

/// 依据目录分组构建候选树；子节点保留完整层级：合集 -> 书 -> 章节。
/// 是否弹出导入对话框由 `inspect_book` 结合层级/标题判断。
fn build_candidates(
    entries: &[NavEntry],
    base_dir: &str,
    path_to_index: &HashMap<String, i64>,
) -> Vec<BookCandidate> {
    let out: Vec<BookCandidate> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| {
            candidate_node(entry, base_dir, path_to_index, format!("{i}"), 0, i + 1)
        })
        .collect();
    out
}

fn candidate_node(
    entry: &NavEntry,
    base_dir: &str,
    path_to_index: &HashMap<String, i64>,
    id: String,
    depth: usize,
    fallback_index: usize,
) -> Option<BookCandidate> {
    let mut indices: Vec<i64> = Vec::new();
    collect_indices(entry, base_dir, path_to_index, &mut indices);
    indices.sort_unstable();
    indices.dedup();
    let (start, end) = (indices.first().copied()?, indices.last().copied()?);
    let children: Vec<BookCandidate> = entry
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, child)| {
            candidate_node(
                child,
                base_dir,
                path_to_index,
                format!("{id}.{i}"),
                depth + 1,
                i + 1,
            )
        })
        .collect();
    let title = entry.title.trim();
    Some(BookCandidate {
        id,
        title: if title.is_empty() {
            format!("第 {fallback_index} 部")
        } else {
            title.to_string()
        },
        start_index: start,
        end_index: end,
        chapter_count: (end - start + 1) as usize,
        depth,
        children,
    })
}

fn collect_indices(
    entry: &NavEntry,
    base_dir: &str,
    path_to_index: &HashMap<String, i64>,
    out: &mut Vec<i64>,
) {
    if let Some(href) = &entry.href {
        let clean = href.split('#').next().unwrap_or(href);
        if !clean.is_empty() {
            let full = resolve_path(base_dir, clean);
            if let Some(i) = path_to_index.get(&full) {
                out.push(*i);
            }
        }
    }
    for child in &entry.children {
        collect_indices(child, base_dir, path_to_index, out);
    }
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

fn parse_opf(bytes: &[u8]) -> Opf {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut opf = Opf {
        title: None,
        titles: Vec::new(),
        author: None,
        manifest: HashMap::new(),
        spine: Vec::new(),
        nav_href: None,
        ncx_href: None,
    };

    let mut in_metadata = false;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local = local_name(e.name().as_ref());
                if local == "metadata" {
                    in_metadata = true;
                }
                if local == "item" {
                    let mut id = None;
                    let mut href = None;
                    let mut media_type = String::new();
                    let mut properties = String::new();
                    for attr in e.attributes().flatten() {
                        let key = local_name(attr.key.as_ref());
                        let val = attr.unescape_value().unwrap_or_default().into_owned();
                        match key.as_str() {
                            "id" => id = Some(val),
                            "href" => href = Some(val),
                            "media-type" => media_type = val,
                            "properties" => properties = val,
                            _ => {}
                        }
                    }
                    if let (Some(id), Some(href)) = (id, href) {
                        if properties.split_whitespace().any(|p| p == "nav") {
                            opf.nav_href = Some(href.clone());
                        }
                        if media_type == "application/x-dtbncx+xml" {
                            opf.ncx_href = Some(href.clone());
                        }
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
                            "title" => {
                                if opf.title.is_none() {
                                    opf.title = Some(value.clone());
                                }
                                opf.titles.push(value);
                            }
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
    opf
}

/// 解析 EPUB3 `nav.xhtml` 中 `epub:type="toc"` 的嵌套列表。
fn parse_nav(bytes: &[u8]) -> Vec<NavEntry> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut roots: Vec<NavEntry> = Vec::new();
    let mut stack: Vec<NavEntry> = Vec::new();
    let mut in_toc = false;
    let mut in_label = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "nav" => {
                        let kind = attr_value(&e, "type").unwrap_or_default();
                        in_toc = kind.is_empty() || kind == "toc";
                    }
                    "li" if in_toc => stack.push(NavEntry::default()),
                    "a" | "span" if in_toc => {
                        if let Some(entry) = stack.last_mut() {
                            if let Some(href) = attr_value(&e, "href") {
                                entry.href = Some(href);
                            }
                        }
                        in_label = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if in_toc && in_label {
                    if let Some(entry) = stack.last_mut() {
                        entry.title.push_str(&t.unescape().unwrap_or_default());
                    }
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()).as_str() {
                "nav" => in_toc = false,
                "a" | "span" => in_label = false,
                "li" if in_toc => {
                    if let Some(entry) = stack.pop() {
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(entry);
                        } else {
                            roots.push(entry);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    roots
}

/// 解析 EPUB2 `toc.ncx` 的 navMap/navPoint 层级。
fn parse_ncx(bytes: &[u8]) -> Vec<NavEntry> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut roots: Vec<NavEntry> = Vec::new();
    let mut stack: Vec<NavEntry> = Vec::new();
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "navPoint" => stack.push(NavEntry::default()),
                    "text" => in_text = true,
                    "content" => {
                        if let (Some(entry), Some(src)) = (stack.last_mut(), attr_value(&e, "src"))
                        {
                            entry.href = Some(src);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "content" {
                    if let (Some(entry), Some(src)) = (stack.last_mut(), attr_value(&e, "src")) {
                        entry.href = Some(src);
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if in_text {
                    if let Some(entry) = stack.last_mut() {
                        entry.title.push_str(&t.unescape().unwrap_or_default());
                    }
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()).as_str() {
                "text" => in_text = false,
                "navPoint" => {
                    if let Some(entry) = stack.pop() {
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(entry);
                        } else {
                            roots.push(entry);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    roots
}

fn find_rootfile(container_xml: &[u8]) -> Option<String> {
    let mut reader = Reader::from_reader(container_xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "rootfile" {
                    return attr_value(&e, "full-path");
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

fn extract_html_title(xhtml: &str, book_title: Option<&str>) -> Option<String> {
    // 章节文件里的 <title> 经常是书名，优先读取正文标题（h1/h2/h3）。
    // 同一文件可能同时有“卷名”和“章名”，按标题形态选出更像章节的标题。
    let lower = xhtml.to_ascii_lowercase();
    let mut headings = Vec::new();
    for (open, close) in [("<h1", "</h1>"), ("<h2", "</h2>"), ("<h3", "</h3>")] {
        if let Some(text) = find_tag_text(xhtml, &lower, open, close) {
            if let Some(title) = normalize_chapter_title(&text, book_title) {
                headings.push(title);
            }
        }
    }
    if !headings.is_empty() {
        let mut best: Option<(i32, String)> = None;
        for title in headings {
            let score = heading_score(&title);
            if best
                .as_ref()
                .map_or(true, |(best_score, _)| score > *best_score)
            {
                best = Some((score, title));
            }
        }
        if let Some((_, title)) = best {
            return Some(title);
        }
    }

    if let Some(text) = find_tag_text(xhtml, &lower, "<title", "</title>") {
        return normalize_chapter_title(&text, book_title);
    }
    None
}

fn find_tag_text(xhtml: &str, lower: &str, open: &str, close: &str) -> Option<String> {
    let start = lower.find(open)?;
    let gt_rel = lower[start..].find('>')?;
    let inner_start = start + gt_rel + 1;
    let end_rel = lower[inner_start..].find(close)?;
    let inner = &xhtml[inner_start..inner_start + end_rel];
    Some(strip_html(inner))
}

fn heading_score(title: &str) -> i32 {
    let t = title.trim();
    if t.is_empty() {
        return -1;
    }
    if t.starts_with('第')
        && (t.contains('章') || t.contains('回') || t.contains('节') || t.contains('话'))
    {
        return 4;
    }
    if [
        "序", "楔子", "引子", "前言", "后记", "尾声", "终章", "番外", "外传", "结局",
    ]
    .iter()
    .any(|p| t.starts_with(p))
    {
        return 3;
    }
    if t.starts_with('第') && (t.contains('卷') || t.contains('部') || t.contains('篇')) {
        return 1;
    }
    let lower = t.to_lowercase();
    if lower.starts_with("chapter")
        || lower.starts_with("prologue")
        || lower.starts_with("epilogue")
    {
        return 3;
    }
    0
}

/// 清洗标题：去掉“书名 - 第一章”“第一章 | 书名”等书名前后缀；纯书名视为无标题。
fn normalize_chapter_title(candidate: &str, book_title: Option<&str>) -> Option<String> {
    let title = candidate.trim();
    if title.is_empty() || title.chars().count() > 60 {
        return None;
    }
    let Some(book_title) = book_title.map(str::trim).filter(|t| !t.is_empty()) else {
        return Some(title.to_string());
    };
    let book_key = title_key(book_title);
    if title_key(title) == book_key {
        return None;
    }
    for sep in [" - ", " – ", " — ", " | ", "·", "_", "：", ":"] {
        let Some((left, right)) = title.split_once(sep) else {
            continue;
        };
        let left = left.trim();
        let right = right.trim();
        if left.is_empty() || right.is_empty() {
            continue;
        }
        if title_key(left) == book_key {
            return Some(right.to_string());
        }
        if title_key(right) == book_key {
            return Some(left.to_string());
        }
    }
    Some(title.to_string())
}

fn title_key(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn attr_value(e: &BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == name {
            return Some(attr.unescape_value().unwrap_or_default().into_owned());
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

fn dir_of(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default()
}

fn resolve_path(dir: &str, href: &str) -> String {
    if dir.is_empty() {
        return normalize_path(href);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn add_file(zw: &mut ZipWriter<Cursor<&mut Vec<u8>>>, name: &str, content: &str) {
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zw.start_file(name, opts).unwrap();
        zw.write_all(content.as_bytes()).unwrap();
    }

    fn build_epub(opf: &str, files: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut zw = ZipWriter::new(cursor);
            let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

            zw.start_file("mimetype", stored).unwrap();
            zw.write_all(b"application/epub+zip").unwrap();

            add_file(
                &mut zw,
                "META-INF/container.xml",
                r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
            );
            add_file(&mut zw, "OEBPS/content.opf", opf);
            for (name, content) in files {
                add_file(&mut zw, name, content);
            }
            zw.finish().unwrap();
        }
        buf
    }

    fn chapter_xhtml(title: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>{title}</title></head>
<body><h1>{title}</h1><p>这是一段足够长的正文内容，用于通过最小长度校验，共计数十个汉字。</p></body></html>"#
        )
    }

    #[test]
    fn parses_in_memory_epub() {
        let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>测试之书</dc:title>
    <dc:creator>作者甲</dc:creator>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
        let bytes = build_epub(opf, &[("OEBPS/ch1.xhtml", &chapter_xhtml("第一章 起"))]);

        let book = parse(&bytes, "fallback").unwrap();
        assert_eq!(book.title, "测试之书");
        assert_eq!(book.author.as_deref(), Some("作者甲"));
        assert_eq!(book.format, "EPUB");
        assert_eq!(book.chapters.len(), 1);
        assert_eq!(book.chapters[0].title, "第一章 起");
        assert!(book.chapters[0].text.contains("足够长的正文内容"));
        assert_eq!(book.chapters[0].content_hash.len(), 64);
        assert!(book.candidates.is_empty());
    }

    #[test]
    fn detects_multibook_via_epub3_nav() {
        let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>合集</dc:title></metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="a1" href="a1.xhtml" media-type="application/xhtml+xml"/>
    <item id="a2" href="a2.xhtml" media-type="application/xhtml+xml"/>
    <item id="b1" href="b1.xhtml" media-type="application/xhtml+xml"/>
    <item id="b2" href="b2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="a1"/><itemref idref="a2"/><itemref idref="b1"/><itemref idref="b2"/>
  </spine>
</package>"#;
        let nav = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<body><nav epub:type="toc"><ol>
  <li><a href="a1.xhtml">上卷 · 风起</a><ol><li><a href="a2.xhtml">第二章</a></li></ol></li>
  <li><a href="b1.xhtml">下卷 · 云涌</a><ol><li><a href="b2.xhtml">第四章</a></li></ol></li>
</ol></nav></body></html>"#;
        let bytes = build_epub(
            opf,
            &[
                ("OEBPS/nav.xhtml", nav),
                ("OEBPS/a1.xhtml", &chapter_xhtml("第一章")),
                ("OEBPS/a2.xhtml", &chapter_xhtml("第二章")),
                ("OEBPS/b1.xhtml", &chapter_xhtml("第三章")),
                ("OEBPS/b2.xhtml", &chapter_xhtml("第四章")),
            ],
        );

        let book = parse(&bytes, "fallback").unwrap();
        assert_eq!(book.chapters.len(), 4);
        assert_eq!(book.candidates.len(), 2);
        assert_eq!(book.candidates[0].title, "上卷 · 风起");
        assert_eq!(book.candidates[0].start_index, 0);
        assert_eq!(book.candidates[0].end_index, 1);
        assert_eq!(book.candidates[0].children.len(), 1);
        assert_eq!(book.candidates[0].children[0].title, "第二章");
        assert_eq!(book.candidates[0].children[0].depth, 1);
        assert_eq!(book.candidates[1].title, "下卷 · 云涌");
        assert_eq!(book.candidates[1].start_index, 2);
        assert_eq!(book.candidates[1].end_index, 3);
    }

    #[test]
    fn builds_nested_collection_book_chapter_tree() {
        let entries = vec![
            NavEntry {
                title: "合集一".into(),
                href: Some("c1.xhtml".into()),
                children: vec![NavEntry {
                    title: "书一".into(),
                    href: Some("b1.xhtml".into()),
                    children: vec![NavEntry {
                        title: "第一章".into(),
                        href: Some("ch1.xhtml".into()),
                        children: vec![],
                    }],
                }],
            },
            NavEntry {
                title: "合集二".into(),
                href: Some("c2.xhtml".into()),
                children: vec![],
            },
            NavEntry {
                title: "合集三".into(),
                href: Some("c3.xhtml".into()),
                children: vec![],
            },
        ];
        let mut map = HashMap::new();
        for (i, name) in ["c1.xhtml", "b1.xhtml", "ch1.xhtml", "c2.xhtml", "c3.xhtml"]
            .iter()
            .enumerate()
        {
            map.insert((*name).to_string(), i as i64);
        }

        let candidates = build_candidates(&entries, "", &map);
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].id, "0");
        assert_eq!(candidates[0].depth, 0);
        assert_eq!(candidates[0].children[0].id, "0.0");
        assert_eq!(candidates[0].children[0].depth, 1);
        assert_eq!(candidates[0].children[0].children[0].id, "0.0.0");
        assert_eq!(candidates[0].children[0].children[0].depth, 2);
        assert_eq!(candidates[0].children[0].children[0].title, "第一章");
    }

    #[test]
    fn keeps_single_collection_root_for_nested_import() {
        let entries = vec![NavEntry {
            title: "全集".into(),
            href: Some("root.xhtml".into()),
            children: vec![
                NavEntry {
                    title: "书一".into(),
                    href: Some("b1.xhtml".into()),
                    children: vec![NavEntry {
                        title: "第一章".into(),
                        href: Some("c1.xhtml".into()),
                        children: vec![],
                    }],
                },
                NavEntry {
                    title: "书二".into(),
                    href: Some("b2.xhtml".into()),
                    children: vec![NavEntry {
                        title: "第二章".into(),
                        href: Some("c2.xhtml".into()),
                        children: vec![],
                    }],
                },
            ],
        }];
        let mut map = HashMap::new();
        for (i, name) in ["root.xhtml", "b1.xhtml", "c1.xhtml", "b2.xhtml", "c2.xhtml"]
            .iter()
            .enumerate()
        {
            map.insert((*name).to_string(), i as i64);
        }
        let candidates = build_candidates(&entries, "", &map);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "全集");
        assert_eq!(candidates[0].children.len(), 2);
        assert_eq!(candidates[0].children[0].children[0].title, "第一章");
    }

    #[test]
    fn detects_multibook_via_ncx() {
        let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>合集</dc:title></metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="a1" href="a1.xhtml" media-type="application/xhtml+xml"/>
    <item id="b1" href="b1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="a1"/><itemref idref="b1"/>
  </spine>
</package>"#;
        let ncx = r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"><navMap>
  <navPoint id="n1"><navLabel><text>第一本书</text></navLabel><content src="a1.xhtml"/></navPoint>
  <navPoint id="n2"><navLabel><text>第二本书</text></navLabel><content src="b1.xhtml"/></navPoint>
</navMap></ncx>"#;
        let bytes = build_epub(
            opf,
            &[
                ("OEBPS/toc.ncx", ncx),
                ("OEBPS/a1.xhtml", &chapter_xhtml("甲")),
                ("OEBPS/b1.xhtml", &chapter_xhtml("乙")),
            ],
        );

        let book = parse(&bytes, "fallback").unwrap();
        assert_eq!(book.candidates.len(), 2);
        assert_eq!(book.candidates[0].title, "第一本书");
        assert_eq!(book.candidates[1].title, "第二本书");
    }

    #[test]
    fn extract_html_title_reads_heading_and_title_tag() {
        assert_eq!(
            extract_html_title("<html><body><h1>第一章 起</h1><p>x</p></body></html>", None)
                .as_deref(),
            Some("第一章 起")
        );
        assert_eq!(
            extract_html_title("<head><title>书名 · 第一章</title></head>", None).as_deref(),
            Some("书名 · 第一章")
        );
    }

    #[test]
    fn extract_html_title_prefers_chapter_heading_over_book_title() {
        let xhtml =
            "<html><head><title>测试之书</title></head><body><h1>第一章 起</h1></body></html>";
        assert_eq!(
            extract_html_title(xhtml, Some("测试之书")).as_deref(),
            Some("第一章 起")
        );
    }

    #[test]
    fn extract_html_title_skips_book_title_and_splits_combined_titles() {
        assert_eq!(
            extract_html_title("<head><title>测试之书</title></head>", Some("测试之书")),
            None
        );
        assert_eq!(
            extract_html_title(
                "<head><title>测试之书 - 第二章 落</title></head>",
                Some("测试之书")
            )
            .as_deref(),
            Some("第二章 落")
        );
        assert_eq!(
            extract_html_title(
                "<head><title>第三章 归 | 测试之书</title></head>",
                Some("测试之书")
            )
            .as_deref(),
            Some("第三章 归")
        );
    }

    #[test]
    fn extract_html_title_prefers_chapter_over_volume_heading() {
        let xhtml = "<body><h1>第一卷 风起</h1><h2>第一章 入山</h2></body>";
        assert_eq!(
            extract_html_title(xhtml, Some("测试之书")).as_deref(),
            Some("第一章 入山")
        );
    }

    #[test]
    fn extract_html_title_does_not_panic_on_long_multibyte() {
        // 修复前：在字节 400 处切片，而 400 落在多字节字符「雾」内部 -> panic。
        let xhtml = format!("<h1>{}</h1>", "雾".repeat(300));
        assert!(extract_html_title(&xhtml, None).is_none());
    }
}
