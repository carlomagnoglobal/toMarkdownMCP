use anyhow::{anyhow, Result};
use std::path::Path;

use crate::html_converter::html_to_markdown_with_options;

/// Extensions handled by this module.
pub fn is_feed_email_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "eml" | "msg" | "epub" | "mobi" | "azw" | "azw3" | "rss" | "atom" | "feed"
    )
}

/// Convert an email, ebook, or feed file to Markdown.
pub fn convert_feed_email(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "eml" => convert_eml(path),
        "msg" => convert_msg(path),
        "epub" => convert_epub(path),
        "mobi" | "azw" | "azw3" => convert_mobi(path),
        "rss" | "atom" | "feed" => convert_feed(path),
        other => Err(anyhow!("Unsupported feed/email extension: {}", other)),
    }
}

/// Render an HTML fragment through the shared HTML→Markdown pipeline; fall back
/// to the raw text on error.
fn html_to_md(html: &str) -> String {
    html_to_markdown_with_options(html, false, false).unwrap_or_else(|_| html.to_string())
}

// ----------------------------------------------------------------------------
// EML (RFC 822 email) via mail-parser
// ----------------------------------------------------------------------------

fn convert_eml(path: &Path) -> Result<String> {
    use mail_parser::MessageParser;

    let bytes = std::fs::read(path).map_err(|e| anyhow!("Failed to read EML: {}", e))?;
    let message = MessageParser::default()
        .parse(&bytes)
        .ok_or_else(|| anyhow!("Failed to parse email"))?;

    let mut out = String::new();

    // Headers as YAML frontmatter.
    out.push_str("---\n");
    if let Some(subject) = message.subject() {
        out.push_str(&format!("subject: {}\n", yaml_escape(subject)));
    }
    let from = format_addresses(message.from());
    if !from.is_empty() {
        out.push_str(&format!("from: {}\n", yaml_escape(&from)));
    }
    let to = format_addresses(message.to());
    if !to.is_empty() {
        out.push_str(&format!("to: {}\n", yaml_escape(&to)));
    }
    if let Some(date) = message.date() {
        out.push_str(&format!("date: {}\n", date));
    }
    out.push_str("---\n\n");

    if let Some(subject) = message.subject() {
        out.push_str(&format!("# {}\n\n", subject));
    }

    // Prefer HTML body, fall back to plain text.
    if let Some(html) = message.body_html(0) {
        out.push_str(&html_to_md(&html));
    } else if let Some(text) = message.body_text(0) {
        out.push_str(&text);
    } else {
        out.push_str("_(no body content)_");
    }
    out.push('\n');
    Ok(out)
}

/// Format a mail-parser address header into a readable string.
fn format_addresses(addr: Option<&mail_parser::Address>) -> String {
    let Some(addr) = addr else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    for a in addr.iter() {
        match (a.name(), a.address()) {
            (Some(name), Some(email)) => parts.push(format!("{} <{}>", name, email)),
            (None, Some(email)) => parts.push(email.to_string()),
            (Some(name), None) => parts.push(name.to_string()),
            (None, None) => {}
        }
    }
    parts.join(", ")
}

fn yaml_escape(s: &str) -> String {
    if s.contains([':', '#', '\n', '"', '\'']) {
        format!("\"{}\"", s.replace('"', "\\\"").replace('\n', " "))
    } else {
        s.to_string()
    }
}

// ----------------------------------------------------------------------------
// MSG (Outlook) — best-effort
// ----------------------------------------------------------------------------

fn convert_msg(path: &Path) -> Result<String> {
    use msg_parser::Outlook;

    let msg = Outlook::from_path(path).map_err(|e| anyhow!("Failed to parse MSG: {:?}", e))?;

    let mut out = String::new();
    out.push_str("---\n");
    if !msg.subject.is_empty() {
        out.push_str(&format!("subject: {}\n", yaml_escape(&msg.subject)));
    }
    let from = format_person(&msg.sender);
    if !from.is_empty() {
        out.push_str(&format!("from: {}\n", yaml_escape(&from)));
    }
    let to = msg.to.iter().map(format_person).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
    if !to.is_empty() {
        out.push_str(&format!("to: {}\n", yaml_escape(&to)));
    }
    out.push_str("---\n\n");

    if !msg.subject.is_empty() {
        out.push_str(&format!("# {}\n\n", msg.subject));
    }
    if msg.body.trim().is_empty() {
        out.push_str("_(no body content)_\n");
    } else {
        out.push_str(msg.body.trim());
        out.push('\n');
    }
    Ok(out)
}

/// Format a msg_parser Person (name + email) into a readable string.
fn format_person(p: &msg_parser::Person) -> String {
    match (p.name.trim(), p.email.trim()) {
        ("", "") => String::new(),
        ("", email) => email.to_string(),
        (name, "") => name.to_string(),
        (name, email) => format!("{} <{}>", name, email),
    }
}

// ----------------------------------------------------------------------------
// EPUB via epub crate
// ----------------------------------------------------------------------------

fn convert_epub(path: &Path) -> Result<String> {
    use epub::doc::EpubDoc;

    let mut doc = EpubDoc::new(path).map_err(|e| anyhow!("Failed to open EPUB: {}", e))?;

    let mut out = String::new();
    if let Some(title) = doc.mdata("title") {
        out.push_str(&format!("# {}\n\n", title.value));
    }
    if let Some(author) = doc.mdata("creator") {
        out.push_str(&format!("_by {}_\n\n", author.value));
    }

    let num_chapters = doc.get_num_chapters();
    for i in 0..num_chapters {
        if doc.set_current_chapter(i) {
            if let Some((content, _mime)) = doc.get_current_str() {
                let md = html_to_md(&content);
                if !md.trim().is_empty() {
                    out.push_str(&md);
                    out.push_str("\n\n");
                }
            }
        }
    }

    if out.trim().is_empty() {
        return Ok("_(no readable content in EPUB)_\n".to_string());
    }
    Ok(out)
}

// ----------------------------------------------------------------------------
// MOBI via mobi crate
// ----------------------------------------------------------------------------

fn convert_mobi(path: &Path) -> Result<String> {
    use mobi::Mobi;

    let m = Mobi::from_path(path).map_err(|e| anyhow!("Failed to open MOBI: {}", e))?;
    let title = m.title();
    let content = m.content_as_string_lossy();

    let mut out = String::new();
    if !title.is_empty() {
        out.push_str(&format!("# {}\n\n", title));
    }
    // MOBI content is typically HTML.
    let md = html_to_md(&content);
    if md.trim().is_empty() {
        out.push_str("_(no readable content in MOBI; it may be DRM-protected)_\n");
    } else {
        out.push_str(&md);
        out.push('\n');
    }
    Ok(out)
}

// ----------------------------------------------------------------------------
// RSS / Atom feeds via feed-rs
// ----------------------------------------------------------------------------

fn convert_feed(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|e| anyhow!("Failed to read feed: {}", e))?;
    feed_bytes_to_markdown(&bytes)
}

/// Convert raw RSS/Atom bytes to Markdown. Reused by `convert_from_source` for
/// feed URLs.
pub fn feed_bytes_to_markdown(bytes: &[u8]) -> Result<String> {
    use feed_rs::parser;

    let feed = parser::parse(bytes).map_err(|e| anyhow!("Failed to parse feed: {}", e))?;

    let mut out = String::new();
    if let Some(title) = &feed.title {
        out.push_str(&format!("# {}\n\n", title.content));
    }
    if let Some(desc) = &feed.description {
        out.push_str(&format!("{}\n\n", desc.content));
    }

    for entry in &feed.entries {
        let title = entry
            .title
            .as_ref()
            .map(|t| t.content.clone())
            .unwrap_or_else(|| "(untitled)".to_string());
        out.push_str(&format!("## {}\n\n", title));

        if let Some(link) = entry.links.first() {
            out.push_str(&format!("[{}]({})\n\n", link.href, link.href));
        }
        if let Some(published) = entry.published {
            out.push_str(&format!("_Published: {}_\n\n", published));
        }

        // Prefer full content, fall back to summary.
        if let Some(content) = &entry.content {
            if let Some(body) = &content.body {
                out.push_str(&html_to_md(body));
                out.push_str("\n\n");
            }
        } else if let Some(summary) = &entry.summary {
            out.push_str(&html_to_md(&summary.content));
            out.push_str("\n\n");
        }
    }

    if out.trim().is_empty() {
        return Ok("_(empty feed)_\n".to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_escape() {
        assert_eq!(yaml_escape("plain"), "plain");
        assert_eq!(yaml_escape("has: colon"), "\"has: colon\"");
    }

    #[test]
    fn test_feed_bytes_to_markdown_rss() {
        let rss = r#"<?xml version="1.0"?>
        <rss version="2.0"><channel>
          <title>My Blog</title>
          <description>Test feed</description>
          <item>
            <title>First Post</title>
            <link>https://example.com/1</link>
            <description>Hello world</description>
          </item>
          <item>
            <title>Second Post</title>
            <link>https://example.com/2</link>
            <description>More content</description>
          </item>
        </channel></rss>"#;
        let md = feed_bytes_to_markdown(rss.as_bytes()).unwrap();
        assert!(md.contains("# My Blog"), "got: {md}");
        assert!(md.contains("## First Post"), "got: {md}");
        assert!(md.contains("## Second Post"), "got: {md}");
        assert!(md.contains("https://example.com/1"), "got: {md}");
    }
}
