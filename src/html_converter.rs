use anyhow::{anyhow, Result};
use scraper::{Html, Selector};

/// Convert HTML content to Markdown (basic conversion)
#[allow(dead_code)]
pub fn html_to_markdown(html_content: &str) -> Result<String> {
    // Use html2md for basic conversion
    let mut markdown = html2md::parse_html(html_content);

    // Clean up common artifacts
    markdown = cleanup_markdown(&markdown);

    Ok(markdown)
}

/// Convert HTML content with enhanced formatting
pub fn html_to_markdown_enhanced(html_content: &str) -> Result<String> {
    let document = Html::parse_document(html_content);
    let mut markdown = String::new();

    // Extract title if present
    let title_selector = Selector::parse("title").map_err(|_| anyhow!("Invalid selector"))?;
    if let Some(title_elem) = document.select(&title_selector).next() {
        let title = title_elem.inner_html();
        if !title.is_empty() {
            markdown.push_str(&format!("# {}\n\n", title));
        }
    }

    // Extract main content
    let body_selector = Selector::parse("body").map_err(|_| anyhow!("Invalid selector"))?;
    let body = if let Some(body_elem) = document.select(&body_selector).next() {
        body_elem.inner_html()
    } else {
        // Fallback to full document if no body tag
        document.root_element().inner_html()
    };

    // Convert body HTML to markdown
    let body_markdown = html2md::parse_html(&body);
    markdown.push_str(&cleanup_markdown(&body_markdown));

    Ok(markdown)
}

/// Extract text content from HTML
#[allow(dead_code)]
pub fn html_to_text(html_content: &str) -> Result<String> {
    let document = Html::parse_document(html_content);

    let mut text = String::new();
    let body_selector = Selector::parse("body").map_err(|_| anyhow!("Invalid selector"))?;
    let body = if let Some(body_elem) = document.select(&body_selector).next() {
        body_elem.clone()
    } else {
        document.root_element().clone()
    };

    for node in body.descendants() {
        if let scraper::node::Node::Text(text_node) = node.value() {
            let text_content = text_node.trim();
            if !text_content.is_empty() {
                text.push_str(text_content);
                text.push(' ');
            }
        }
    }

    Ok(text.trim().to_string())
}

/// Clean up Markdown output
fn cleanup_markdown(markdown: &str) -> String {
    let mut result = String::new();
    let mut prev_blank = false;

    for line in markdown.lines() {
        let trimmed = line.trim();

        // Skip multiple consecutive blank lines
        if trimmed.is_empty() {
            if !prev_blank {
                result.push('\n');
                prev_blank = true;
            }
        } else {
            // Remove excess spaces
            let cleaned = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");

            // Add line with consistent formatting
            result.push_str(&cleaned);
            result.push('\n');
            prev_blank = false;
        }
    }

    // Remove trailing whitespace
    result.trim_end().to_string() + "\n"
}

/// Extract metadata from HTML
#[allow(dead_code)]
pub fn extract_html_metadata(html_content: &str) -> Result<HtmlMetadata> {
    let document = Html::parse_document(html_content);
    let mut metadata = HtmlMetadata::default();

    // Extract title
    let title_selector = Selector::parse("title").map_err(|_| anyhow!("Invalid selector"))?;
    if let Some(title_elem) = document.select(&title_selector).next() {
        metadata.title = Some(title_elem.inner_html());
    }

    // Extract meta description
    let meta_selector = Selector::parse("meta[name=\"description\"]")
        .map_err(|_| anyhow!("Invalid selector"))?;
    if let Some(meta_elem) = document.select(&meta_selector).next() {
        if let Some(content) = meta_elem.value().attr("content") {
            metadata.description = Some(content.to_string());
        }
    }

    // Extract headings
    let h1_selector = Selector::parse("h1").map_err(|_| anyhow!("Invalid selector"))?;
    for h1 in document.select(&h1_selector) {
        metadata.headings.push(h1.inner_html());
    }

    // Count various elements
    let p_selector = Selector::parse("p").map_err(|_| anyhow!("Invalid selector"))?;
    metadata.paragraph_count = document.select(&p_selector).count();

    let a_selector = Selector::parse("a").map_err(|_| anyhow!("Invalid selector"))?;
    metadata.link_count = document.select(&a_selector).count();

    let img_selector = Selector::parse("img").map_err(|_| anyhow!("Invalid selector"))?;
    metadata.image_count = document.select(&img_selector).count();

    let code_selector = Selector::parse("code").map_err(|_| anyhow!("Invalid selector"))?;
    metadata.code_block_count = document.select(&code_selector).count();

    Ok(metadata)
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct HtmlMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub headings: Vec<String>,
    pub paragraph_count: usize,
    pub link_count: usize,
    pub image_count: usize,
    pub code_block_count: usize,
}

/// Detect HTML format and extract the HTML content
pub fn extract_html_from_mhtml(mhtml_content: &str) -> Result<String> {
    // MHTML format: HTTP headers followed by MIME multipart message
    // Find the boundary marker
    let boundary = mhtml_content
        .lines()
        .find(|line| line.contains("boundary="))
        .and_then(|line| {
            line.split("boundary=\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
        })
        .ok_or_else(|| anyhow!("No MIME boundary found in MHTML"))?;

    let boundary_marker = format!("--{}", boundary);

    // Find the HTML part (usually first part after boundary)
    let parts: Vec<&str> = mhtml_content.split(&boundary_marker).collect();

    for part in parts.iter().skip(1) {
        // Check if this is the HTML part
        if part.contains("Content-Type: text/html") {
            // Extract the actual content after headers
            if let Some(content_start) = part.find("\r\n\r\n").or_else(|| part.find("\n\n")) {
                let html_start = if part[content_start..].starts_with("\r\n\r\n") {
                    content_start + 4
                } else {
                    content_start + 2
                };

                let html_content = &part[html_start..];

                // Remove the trailing boundary marker if present
                if let Some(end_pos) = html_content.rfind("\r\n--") {
                    return Ok(html_content[..end_pos].trim().to_string());
                }

                return Ok(html_content.trim().to_string());
            }
        }
    }

    Err(anyhow!("No HTML content found in MHTML file"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_markdown_simple() {
        let html = "<h1>Hello</h1><p>This is a test.</p>";
        let result = html_to_markdown(html).unwrap();
        assert!(result.contains("Hello"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_html_to_text() {
        let html = "<h1>Hello</h1><p>This is a test.</p>";
        let result = html_to_text(html).unwrap();
        assert!(result.contains("Hello"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_cleanup_markdown() {
        let markdown = "# Title\n\n\n\nSome text\n\n\nMore text";
        let result = cleanup_markdown(markdown);
        // Should remove multiple consecutive blank lines
        assert!(!result.contains("\n\n\n"));
    }
}
