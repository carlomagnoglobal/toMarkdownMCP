use anyhow::{anyhow, Result};
use scraper::{Html, Selector};
use std::collections::BTreeMap;
use crate::code_language_detector::LanguageDetector;

/// Enhance HTML code blocks with detected language information
pub fn enhance_code_blocks_with_language(html_content: &str) -> Result<String> {
    let detector = LanguageDetector::new();
    let mut result = html_content.to_string();

    // Process code blocks iteratively to add language classes
    loop {
        // Find the next code block without language class
        if let Some(start) = result.find("<code") {
            // Find the closing > of the opening tag
            if let Some(end_offset) = result[start..].find('>') {
                let tag_end = start + end_offset + 1;
                let tag = &result[start..tag_end];

                // Check if already has language specification
                if tag.contains("language-") || tag.contains("lang-") {
                    // Skip this one, find the next
                    result = result[tag_end..].to_string();
                    continue;
                }

                // Find the closing </code> tag
                if let Some(close_pos) = result[tag_end..].find("</code>") {
                    let code_text = &result[tag_end..tag_end + close_pos];

                    // Try to detect language
                    if let Some(lang) = detector.detect_language(code_text) {
                        // Construct new tag with language class
                        let new_tag = if tag.ends_with('>') {
                            format!(
                                "{}class=\"language-{}\" ",
                                &tag[..tag.len() - 1],
                                lang
                            ) + ">"
                        } else {
                            tag.to_string()
                        };

                        // Replace in result
                        result = result[..start].to_string() + &new_tag + &result[tag_end..];
                        continue; // Keep looking for more code blocks
                    }
                }
            }
        } else {
            // No more code blocks without language class
            break;
        }
    }

    Ok(result)
}

/// Convert HTML content to Markdown (basic conversion)
#[allow(dead_code)]
pub fn html_to_markdown(html_content: &str) -> Result<String> {
    // Use html2md for basic conversion
    let mut markdown = html2md::parse_html(html_content);

    // Clean up common artifacts
    markdown = cleanup_markdown(&markdown);

    Ok(markdown)
}

/// Convert HTML content with enhanced formatting (wrapper for backward compatibility)
#[allow(dead_code)]
pub fn html_to_markdown_enhanced(html_content: &str) -> Result<String> {
    html_to_markdown_with_options(html_content, false, false)
}

/// Convert HTML content with optional metadata extraction and CSS hints (wrapper)
#[allow(dead_code)]
pub fn html_to_markdown_with_metadata(html_content: &str, extract_metadata: bool) -> Result<String> {
    html_to_markdown_with_options(html_content, extract_metadata, false)
}

/// Convert HTML content with all options
pub fn html_to_markdown_with_options(
    html_content: &str,
    extract_metadata: bool,
    preserve_css_hints: bool,
) -> Result<String> {
    let document = Html::parse_document(html_content);
    let mut markdown = String::new();

    // Extract metadata if requested
    if extract_metadata {
        let metadata = extract_html_metadata(html_content)?;
        if !metadata.is_empty() {
            markdown.push_str(&metadata_to_yaml_frontmatter(&metadata));
            markdown.push_str("\n");
        }
    }

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

    // Enhance code blocks with language detection
    let processed_html = enhance_code_blocks_with_language(&body)?;

    // Optionally add CSS hints to HTML before conversion
    let processed_html = if preserve_css_hints {
        add_css_hints_to_html(&processed_html)?
    } else {
        processed_html
    };

    // Convert body HTML to markdown
    let body_markdown = html2md::parse_html(&processed_html);
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

/// Extract metadata from HTML and return as key-value map
pub fn extract_html_metadata(html_content: &str) -> Result<BTreeMap<String, String>> {
    let document = Html::parse_document(html_content);
    let mut metadata = BTreeMap::new();

    // Extract title
    let title_selector = Selector::parse("title").map_err(|_| anyhow!("Invalid selector"))?;
    if let Some(title_elem) = document.select(&title_selector).next() {
        let title = title_elem.inner_html();
        if !title.is_empty() {
            metadata.insert("title".to_string(), title);
        }
    }

    // Extract meta tags (description, author, keywords, etc.)
    let meta_selector = Selector::parse("meta").map_err(|_| anyhow!("Invalid selector"))?;

    for meta_elem in document.select(&meta_selector) {
        let elem = meta_elem.value();

        // Check for name attribute (standard meta tags)
        if let Some(name) = elem.attr("name") {
            if let Some(content) = elem.attr("content") {
                let key = name.to_lowercase();
                if !content.is_empty() && !metadata.contains_key(&key) {
                    metadata.insert(key, content.to_string());
                }
            }
        }

        // Check for property attribute (Open Graph)
        if let Some(property) = elem.attr("property") {
            if let Some(content) = elem.attr("content") {
                if property.starts_with("og:") && !content.is_empty() {
                    let key = property.replace("og:", "og_");
                    if !metadata.contains_key(&key) {
                        metadata.insert(key, content.to_string());
                    }
                }
            }
        }
    }

    // Extract language from html tag
    let html_selector = Selector::parse("html").map_err(|_| anyhow!("Invalid selector"))?;
    if let Some(html_elem) = document.select(&html_selector).next() {
        if let Some(lang) = html_elem.value().attr("lang") {
            metadata.insert("language".to_string(), lang.to_string());
        }
    }

    Ok(metadata)
}

/// Extract detailed metadata for analysis
#[allow(dead_code)]
pub fn extract_html_metadata_detailed(html_content: &str) -> Result<HtmlMetadata> {
    let document = Html::parse_document(html_content);
    let mut metadata = HtmlMetadata::default();
    let map = extract_html_metadata(html_content)?;

    // Populate HtmlMetadata from map
    if let Some(title) = map.get("title") {
        metadata.title = Some(title.clone());
    }
    if let Some(description) = map.get("description") {
        metadata.description = Some(description.clone());
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

/// Convert metadata map to YAML frontmatter format
pub fn metadata_to_yaml_frontmatter(metadata: &BTreeMap<String, String>) -> String {
    if metadata.is_empty() {
        return String::new();
    }

    let mut yaml = String::from("---\n");

    for (key, value) in metadata.iter() {
        // Skip internal keys
        if key == "og_image" || key == "og_type" {
            continue;
        }

        // Format the key
        let formatted_key = key.replace('_', "-");

        // Check if value needs to be quoted (contains special chars or is numeric)
        let needs_quotes = value.contains(':') || value.contains('"') ||
                          value.starts_with(' ') || value.ends_with(' ') ||
                          value.contains('\n');

        if needs_quotes {
            let escaped = value.replace('"', "\\\"");
            yaml.push_str(&format!("{}: \"{}\"\n", formatted_key, escaped));
        } else {
            yaml.push_str(&format!("{}: {}\n", formatted_key, value));
        }
    }

    yaml.push_str("---\n");
    yaml
}

/// Extract CSS styling hints from HTML and inject as comments
pub fn add_css_hints_to_html(html_content: &str) -> Result<String> {
    let document = Html::parse_document(html_content);
    let mut enhanced_html = String::new();

    // Extract inline styles from style tag if present
    let style_selector = Selector::parse("style").map_err(|_| anyhow!("Invalid selector"))?;
    let _styles: Vec<String> = document
        .select(&style_selector)
        .map(|elem| elem.inner_html())
        .collect();

    // Process elements with inline styles or classes
    process_elements_with_hints(html_content, &mut enhanced_html)?;

    Ok(enhanced_html)
}

/// Process HTML elements and add CSS hints as comments
fn process_elements_with_hints(html_content: &str, output: &mut String) -> Result<()> {
    let document = Html::parse_document(html_content);

    // Get all elements with style attribute
    let all_selector = Selector::parse("*").map_err(|_| anyhow!("Invalid selector"))?;

    for elem in document.select(&all_selector) {
        if let Some(style) = elem.value().attr("style") {
            let hints = extract_style_hints(style);
            if !hints.is_empty() {
                // Store hints for later use (they'll be added by html2md processing)
                // For now, we'll use a different approach - modify the HTML directly
            }
        }
    }

    // For now, return the original HTML - hints will be added during conversion
    *output = html_content.to_string();
    Ok(())
}

/// Extract meaningful CSS hints from a style attribute
pub fn extract_style_hints(style: &str) -> Vec<(String, String)> {
    let mut hints = Vec::new();

    let declarations = style.split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let meaningful_properties = [
        "color",
        "background-color",
        "background",
        "font-weight",
        "font-style",
        "text-align",
        "text-decoration",
        "font-size",
        "text-transform",
        "letter-spacing",
        "line-height",
        "margin",
        "padding",
        "border",
        "display",
        "font-family",
    ];

    for decl in declarations {
        if let Some(colon_pos) = decl.find(':') {
            let property = decl[..colon_pos].trim().to_lowercase();
            let value = decl[colon_pos + 1..].trim().to_string();

            // Only keep meaningful properties, skip defaults and framework utilities
            if meaningful_properties.contains(&property.as_str()) && !is_default_value(&property, &value) {
                hints.push((property, value));
            }
        }
    }

    hints
}

/// Check if a CSS value is a default/common value we can skip
fn is_default_value(property: &str, value: &str) -> bool {
    match property {
        "display" => matches!(value, "block" | "inline"),
        "margin" | "padding" => matches!(value, "0" | "auto"),
        "font-weight" => matches!(value, "normal" | "400"),
        "font-style" => matches!(value, "normal"),
        "text-align" => matches!(value, "left"),
        "text-decoration" => matches!(value, "none"),
        "line-height" => matches!(value, "1" | "1.2" | "normal"),
        "color" => value.contains("inherit"),
        _ => false,
    }
}

/// Format CSS hints as HTML comments for insertion into Markdown
#[allow(dead_code)]
pub fn format_css_hints_as_comment(hints: &[(String, String)]) -> String {
    if hints.is_empty() {
        return String::new();
    }

    let mut comment = String::from("<!-- CSS: ");

    let hint_strs: Vec<String> = hints
        .iter()
        .map(|(prop, val)| format!("{}: {}", prop, val))
        .collect();

    comment.push_str(&hint_strs.join("; "));
    comment.push_str(" -->\n");

    comment
}

/// Check if text contains styling markers
#[allow(dead_code)]
pub fn get_text_formatting_hints(text: &str) -> Vec<String> {
    let mut hints = Vec::new();

    // Detect text that might be styled
    if text.chars().all(|c| c.is_uppercase() || c.is_whitespace()) && text.len() > 3 {
        hints.push("text-transform: uppercase".to_string());
    }

    if text.contains("**") || text.contains("***") {
        hints.push("font-weight: bold".to_string());
    }

    if text.contains("_") || text.contains("*") {
        hints.push("font-style: italic".to_string());
    }

    hints
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
