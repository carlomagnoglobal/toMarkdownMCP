use anyhow::{anyhow, Result};
use std::io::Read;
use std::path::Path;

use crate::html_converter::html_to_markdown_with_options;

/// Extensions handled by this module.
pub fn is_document_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "pdf" | "docx" | "doc" | "rtf" | "odt"
    )
}

/// Convert a binary/office document to Markdown, dispatching by extension.
pub fn convert_document(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "pdf" => convert_pdf(path),
        "docx" => convert_docx(path),
        "odt" => convert_odt(path),
        "rtf" => convert_rtf(path),
        "doc" => convert_doc(path),
        other => Err(anyhow!("Unsupported document extension: {}", other)),
    }
}

/// Collapse runs of whitespace and trim, preserving paragraph breaks.
fn normalize_text(raw: &str) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    for block in raw.split("\n\n") {
        let collapsed: String = block.split_whitespace().collect::<Vec<_>>().join(" ");
        if !collapsed.is_empty() {
            paragraphs.push(collapsed);
        }
    }
    paragraphs.join("\n\n")
}

// ----------------------------------------------------------------------------
// PDF
// ----------------------------------------------------------------------------

fn convert_pdf(path: &Path) -> Result<String> {
    let text = pdf_extract::extract_text(path)
        .map_err(|e| anyhow!("Failed to extract PDF text: {}", e))?;

    let normalized = normalize_text(&text);
    if normalized.trim().is_empty() {
        return Ok("> **Note:** No extractable text found in this PDF. It may be a scanned/image-only \
             document (OCR is not supported).".to_string());
    }
    Ok(normalized)
}

// ----------------------------------------------------------------------------
// DOCX (Office Open XML)
// ----------------------------------------------------------------------------

/// Read a single entry from a zip archive into a string.
fn read_zip_entry(path: &Path, entry_name: &str) -> Result<String> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow!("Failed to open archive {}: {}", path.display(), e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| anyhow!("Failed to read zip archive: {}", e))?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| anyhow!("Entry '{}' not found in archive: {}", entry_name, e))?;
    let mut contents = String::new();
    entry
        .read_to_string(&mut contents)
        .map_err(|e| anyhow!("Failed to read '{}': {}", entry_name, e))?;
    Ok(contents)
}

fn convert_docx(path: &Path) -> Result<String> {
    let xml = read_zip_entry(path, "word/document.xml")?;
    Ok(docx_xml_to_markdown(&xml))
}

/// Convert the body of a WordprocessingML document.xml to Markdown.
/// Handles heading styles, list paragraphs, and bold/italic runs.
fn docx_xml_to_markdown(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(false);

    let mut out = String::new();
    let mut para_text = String::new();
    let mut para_style: Option<String> = None;
    let mut is_list = false;

    // Run-level formatting state.
    let mut in_run_props = false;
    let mut bold = false;
    let mut italic = false;
    let mut run_bold = false;
    let mut run_italic = false;
    let mut in_text = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                let local = name.local_name();
                match local.as_ref() {
                    b"p" => {
                        para_text.clear();
                        para_style = None;
                        is_list = false;
                    }
                    b"pStyle" => {
                        if let Some(val) = attr_value(&e, b"val") {
                            para_style = Some(val);
                        }
                    }
                    b"numPr" => {
                        is_list = true;
                    }
                    b"r" => {
                        // Reset run formatting at the start of each run; runs
                        // without an explicit <w:rPr> inherit no formatting.
                        run_bold = false;
                        run_italic = false;
                    }
                    b"rPr" => {
                        in_run_props = true;
                        run_bold = false;
                        run_italic = false;
                    }
                    b"b" => {
                        if in_run_props {
                            run_bold = attr_toggle(&e);
                        }
                    }
                    b"i" => {
                        if in_run_props {
                            run_italic = attr_toggle(&e);
                        }
                    }
                    b"t" => {
                        in_text = true;
                        bold = run_bold;
                        italic = run_italic;
                    }
                    b"tab" => para_text.push('\t'),
                    b"br" => para_text.push_str("  \n"),
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if in_text {
                    let text = t.unescape().unwrap_or_default().into_owned();
                    para_text.push_str(&decorate(&text, bold, italic));
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                match name.local_name().as_ref() {
                    b"rPr" => in_run_props = false,
                    b"t" => in_text = false,
                    b"p" => {
                        flush_paragraph(&mut out, &para_text, &para_style, is_list);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    out.trim_end().to_string() + "\n"
}

fn decorate(text: &str, bold: bool, italic: bool) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut s = text.to_string();
    if bold {
        s = format!("**{}**", s);
    }
    if italic {
        s = format!("*{}*", s);
    }
    s
}

/// Emit a completed paragraph with appropriate Markdown prefix.
fn flush_paragraph(out: &mut String, text: &str, style: &Option<String>, is_list: bool) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Some(level) = style.as_deref().and_then(heading_level) {
        out.push_str(&"#".repeat(level));
        out.push(' ');
        out.push_str(trimmed);
        out.push_str("\n\n");
    } else if is_list {
        out.push_str("- ");
        out.push_str(trimmed);
        out.push('\n');
    } else {
        out.push_str(trimmed);
        out.push_str("\n\n");
    }
}

/// Map a Word style name like "Heading1" / "Heading 2" to a heading level.
fn heading_level(style: &str) -> Option<usize> {
    let lower = style.to_lowercase().replace([' ', '-', '_'], "");
    let rest = lower.strip_prefix("heading")?;
    match rest.parse::<usize>() {
        Ok(n) if (1..=6).contains(&n) => Some(n),
        _ => None,
    }
}

/// Read an attribute by local name from a start/empty element.
fn attr_value(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.local_name().as_ref() == key {
            Some(String::from_utf8_lossy(&a.value).into_owned())
        } else {
            None
        }
    })
}

/// Interpret a WordprocessingML toggle property (`<w:b/>`, `<w:b w:val="true"/>`,
/// or `<w:b w:val="false"/>`).
fn attr_toggle(e: &quick_xml::events::BytesStart) -> bool {
    match attr_value(e, b"val") {
        None => true,
        Some(v) => !matches!(v.as_str(), "false" | "0" | "off"),
    }
}

// ----------------------------------------------------------------------------
// ODT (OpenDocument Text)
// ----------------------------------------------------------------------------

fn convert_odt(path: &Path) -> Result<String> {
    let xml = read_zip_entry(path, "content.xml")?;
    Ok(odt_xml_to_markdown(&xml))
}

/// Convert OpenDocument content.xml body to Markdown. Heading levels come from
/// `text:outline-level` on `text:h` elements; `text:p` are paragraphs.
fn odt_xml_to_markdown(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(false);

    let mut out = String::new();
    let mut text = String::new();
    let mut heading_lvl: Option<usize> = None;
    let mut is_list_item = false;
    let mut list_depth = 0usize;
    let mut capture = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().local_name().as_ref() {
                b"h" => {
                    capture = true;
                    text.clear();
                    heading_lvl = attr_value(&e, b"outline-level")
                        .and_then(|v| v.parse::<usize>().ok())
                        .map(|n| n.clamp(1, 6));
                }
                b"p" => {
                    capture = true;
                    text.clear();
                    heading_lvl = None;
                }
                b"list" => list_depth += 1,
                b"list-item" => is_list_item = true,
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if capture {
                    text.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => match e.name().local_name().as_ref() {
                b"h" | b"p" => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        if let Some(lvl) = heading_lvl {
                            out.push_str(&"#".repeat(lvl));
                            out.push(' ');
                            out.push_str(trimmed);
                            out.push_str("\n\n");
                        } else if is_list_item {
                            out.push_str(&"  ".repeat(list_depth.saturating_sub(1)));
                            out.push_str("- ");
                            out.push_str(trimmed);
                            out.push('\n');
                        } else {
                            out.push_str(trimmed);
                            out.push_str("\n\n");
                        }
                    }
                    capture = false;
                }
                b"list" => list_depth = list_depth.saturating_sub(1),
                b"list-item" => is_list_item = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    out.trim_end().to_string() + "\n"
}

// ----------------------------------------------------------------------------
// RTF (best-effort control-word stripper)
// ----------------------------------------------------------------------------

fn convert_rtf(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .map_err(|e| anyhow!("Failed to read RTF file: {}", e))?;
    let raw = String::from_utf8_lossy(&bytes);
    Ok(strip_rtf(&raw))
}

/// Minimal RTF-to-text: drops control words and groups, keeps plain runs and
/// paragraph breaks (`\par`).
fn strip_rtf(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                // Control word or control symbol.
                if let Some(&next) = chars.peek() {
                    if next.is_ascii_alphabetic() {
                        let mut word = String::new();
                        while let Some(&nc) = chars.peek() {
                            if nc.is_ascii_alphabetic() {
                                word.push(nc);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        // Optional numeric parameter.
                        let mut param = String::new();
                        if matches!(chars.peek(), Some('-')) {
                            param.push('-');
                            chars.next();
                        }
                        while let Some(&nc) = chars.peek() {
                            if nc.is_ascii_digit() {
                                param.push(nc);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        // Consume a single trailing space delimiter.
                        if matches!(chars.peek(), Some(' ')) {
                            chars.next();
                        }
                        match word.as_str() {
                            "par" => out.push_str("\n\n"),
                            "line" => out.push('\n'),
                            "tab" => out.push('\t'),
                            _ => {}
                        }
                    } else {
                        // Escaped literal (\{, \}, \\) or symbol; keep braces/backslash.
                        if matches!(next, '{' | '}' | '\\') {
                            out.push(next);
                        }
                        chars.next();
                    }
                }
            }
            '{' | '}' => {} // group boundaries
            '\r' | '\n' => {} // raw line breaks are not significant in RTF
            _ => out.push(c),
        }
    }

    normalize_text(&out)
}

// ----------------------------------------------------------------------------
// DOC (legacy OLE) — best effort
// ----------------------------------------------------------------------------

fn convert_doc(path: &Path) -> Result<String> {
    // Legacy binary .doc (OLE compound file) has no reliable pure-Rust parser.
    // Best effort: pull ASCII/UTF-8 runs out of the binary so at least the body
    // text is recoverable; otherwise return a clear fallback note.
    let bytes = std::fs::read(path)
        .map_err(|e| anyhow!("Failed to read DOC file: {}", e))?;
    let text = extract_printable_runs(&bytes);
    let normalized = normalize_text(&text);
    if normalized.trim().len() < 20 {
        return Ok("> **Note:** This legacy `.doc` file could not be reliably parsed. Please convert it \
             to `.docx` and try again (legacy OLE `.doc` is only best-effort supported).".to_string());
    }
    Ok(normalized)
}

/// Extract runs of printable text (length >= 4) from arbitrary bytes.
fn extract_printable_runs(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut run = String::new();
    for &b in bytes {
        let c = b as char;
        if c == '\n' || c == '\t' || (b' '..=b'~').contains(&b) {
            run.push(c);
        } else {
            if run.trim().len() >= 4 {
                out.push_str(run.trim());
                out.push('\n');
            }
            run.clear();
        }
    }
    if run.trim().len() >= 4 {
        out.push_str(run.trim());
    }
    out
}

/// Helper reused by EPUB/feed converters: wrap an HTML fragment through the
/// shared HTML→Markdown pipeline with default options.
#[allow(dead_code)]
pub fn html_fragment_to_markdown(html: &str) -> Result<String> {
    html_to_markdown_with_options(html, false, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_text_collapses_whitespace() {
        let input = "Hello    world\n\n\nFoo   bar";
        let out = normalize_text(input);
        assert_eq!(out, "Hello world\n\nFoo bar");
    }

    #[test]
    fn test_heading_level_maps_styles() {
        assert_eq!(heading_level("Heading1"), Some(1));
        assert_eq!(heading_level("Heading 3"), Some(3));
        assert_eq!(heading_level("heading-2"), Some(2));
        assert_eq!(heading_level("Normal"), None);
        assert_eq!(heading_level("Heading9"), None);
    }

    #[test]
    fn test_docx_xml_headings_and_bold() {
        let xml = r#"<?xml version="1.0"?>
        <w:document xmlns:w="x">
          <w:body>
            <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>
            <w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Bold text</w:t></w:r></w:p>
            <w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t>Item one</w:t></w:r></w:p>
          </w:body>
        </w:document>"#;
        let md = docx_xml_to_markdown(xml);
        assert!(md.contains("# Title"), "got: {md}");
        assert!(md.contains("**Bold text**"), "got: {md}");
        assert!(md.contains("- Item one"), "got: {md}");
    }

    #[test]
    fn test_odt_xml_headings() {
        let xml = r#"<office xmlns:text="x">
          <text:h text:outline-level="2">Section</text:h>
          <text:p>Body paragraph.</text:p>
        </office>"#;
        let md = odt_xml_to_markdown(xml);
        assert!(md.contains("## Section"), "got: {md}");
        assert!(md.contains("Body paragraph."), "got: {md}");
    }

    #[test]
    fn test_strip_rtf_basic() {
        let rtf = r"{\rtf1\ansi Hello \b bold\b0  world\par Second line\par}";
        let out = strip_rtf(rtf);
        assert!(out.contains("Hello"), "got: {out}");
        assert!(out.contains("bold"), "got: {out}");
        assert!(out.contains("world"), "got: {out}");
        assert!(out.contains("Second line"), "got: {out}");
    }

    #[test]
    fn test_extract_printable_runs() {
        let mut data = vec![0u8, 1, 2];
        data.extend_from_slice(b"Hello world");
        data.extend_from_slice(&[0, 0]);
        data.extend_from_slice(b"Second");
        let out = extract_printable_runs(&data);
        assert!(out.contains("Hello world"));
        assert!(out.contains("Second"));
    }
}
