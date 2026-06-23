use anyhow::{anyhow, Result};
use scraper::Html;

/// Represents an HTML comment
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlComment {
    pub content: String,
    pub is_directive: bool,      // e.g., <!--[if IE]-->
    pub is_conditional: bool,     // IE conditional comments
}

impl HtmlComment {
    /// Check if comment is a directive (like conditional IE comments)
    fn detect_type(content: &str) -> (bool, bool) {
        let trimmed = content.trim();
        let is_directive = trimmed.starts_with('[') || trimmed.contains("DOCTYPE");
        let is_conditional = trimmed.starts_with("[if ") || trimmed.contains("[endif]");
        (is_directive, is_conditional)
    }
}

/// Extract all HTML comments from content
pub fn extract_comments_from_html(html_content: &str) -> Result<Vec<HtmlComment>> {
    let mut comments = Vec::new();

    // Parse as string to find comments since scraper doesn't expose comment nodes directly
    let mut pos = 0;
    while let Some(start) = html_content[pos..].find("<!--") {
        let absolute_start = pos + start;
        if let Some(end) = html_content[absolute_start..].find("-->") {
            let absolute_end = absolute_start + end;
            let comment_content = &html_content[absolute_start + 4..absolute_end];

            let (is_directive, is_conditional) = HtmlComment::detect_type(comment_content);

            comments.push(HtmlComment {
                content: comment_content.to_string(),
                is_directive,
                is_conditional,
            });

            pos = absolute_end + 3;
        } else {
            break;
        }
    }

    Ok(comments)
}

/// Convert comment to Markdown representation
pub fn comment_to_markdown(comment: &HtmlComment, preserve_as_html: bool) -> String {
    let content = comment.content.trim();

    if preserve_as_html {
        // Preserve as HTML comment in Markdown
        format!("<!-- {} -->", content)
    } else {
        // Convert to visible Markdown note format
        if comment.is_conditional {
            format!("⚠️ **IE Conditional:** {}", content)
        } else if comment.is_directive {
            format!("📋 **Directive:** {}", content)
        } else if content.len() > 100 {
            format!("💬 **Note:** {}...", &content[..100])
        } else {
            format!("💬 **Note:** {}", content)
        }
    }
}

/// Generate summary of comments found in HTML
pub fn generate_comment_summary(comments: &[HtmlComment]) -> String {
    let mut summary = String::new();

    summary.push_str("## Comments Found\n\n");
    summary.push_str(&format!("**Total:** {} comments\n\n", comments.len()));

    let directive_count = comments.iter().filter(|c| c.is_directive).count();
    let conditional_count = comments.iter().filter(|c| c.is_conditional).count();
    let regular_count = comments.len() - directive_count;

    if directive_count > 0 {
        summary.push_str(&format!("**Directives:** {} | ", directive_count));
    }
    if conditional_count > 0 {
        summary.push_str(&format!("**Conditional:** {} | ", conditional_count));
    }
    summary.push_str(&format!("**Regular:** {}\n\n", regular_count));

    // List comments by type
    let directives: Vec<_> = comments.iter().filter(|c| c.is_directive && !c.is_conditional).collect();
    let conditionals: Vec<_> = comments.iter().filter(|c| c.is_conditional).collect();
    let regular: Vec<_> = comments.iter().filter(|c| !c.is_directive).collect();

    if !directives.is_empty() {
        summary.push_str("### Directives\n\n");
        for (i, comment) in directives.iter().enumerate() {
            let content = if comment.content.len() > 80 {
                format!("{}...", &comment.content[..80])
            } else {
                comment.content.clone()
            };
            summary.push_str(&format!("{}. `{}`\n", i + 1, content.trim()));
        }
        summary.push('\n');
    }

    if !conditionals.is_empty() {
        summary.push_str("### Conditional Comments (IE)\n\n");
        for (i, comment) in conditionals.iter().enumerate() {
            let content = if comment.content.len() > 80 {
                format!("{}...", &comment.content[..80])
            } else {
                comment.content.clone()
            };
            summary.push_str(&format!("{}. `{}`\n", i + 1, content.trim()));
        }
        summary.push('\n');
    }

    if !regular.is_empty() {
        summary.push_str("### Regular Comments\n\n");
        for (i, comment) in regular.iter().enumerate() {
            let content = if comment.content.len() > 80 {
                format!("{}...", &comment.content[..80])
            } else {
                comment.content.clone()
            };
            summary.push_str(&format!("{}. `{}`\n", i + 1, content.trim()));
        }
        summary.push('\n');
    }

    summary
}

/// Remove or preserve comments in HTML
pub fn process_comments_in_html(html_content: &str, preserve: bool) -> Result<(String, Vec<HtmlComment>)> {
    let comments = extract_comments_from_html(html_content)?;

    if !preserve {
        // Remove comments from HTML
        let mut result = html_content.to_string();
        let mut pos = 0;

        while let Some(start) = result[pos..].find("<!--") {
            let absolute_start = pos + start;
            if let Some(end) = result[absolute_start..].find("-->") {
                let absolute_end = absolute_start + end + 3;
                result.drain(absolute_start..absolute_end);
                pos = absolute_start;
            } else {
                break;
            }
        }

        Ok((result, comments))
    } else {
        // Keep comments as-is
        Ok((html_content.to_string(), comments))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_comment() {
        let html = r#"<div><!-- This is a comment --></div>"#;
        let comments = extract_comments_from_html(html).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].content, " This is a comment ");
        assert!(!comments[0].is_directive);
    }

    #[test]
    fn test_extract_multiple_comments() {
        let html = r#"
            <!-- Comment 1 -->
            <p>Content</p>
            <!-- Comment 2 -->
        "#;
        let comments = extract_comments_from_html(html).unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].content.trim(), "Comment 1");
        assert_eq!(comments[1].content.trim(), "Comment 2");
    }

    #[test]
    fn test_extract_nested_comments_in_tags() {
        let html = r#"<div><!-- comment before --><p>text</p><!-- comment after --></div>"#;
        let comments = extract_comments_from_html(html).unwrap();
        assert_eq!(comments.len(), 2);
    }

    #[test]
    fn test_detect_conditional_comment() {
        let html = r#"<!--[if IE]><p>IE Only</p><![endif]-->"#;
        let comments = extract_comments_from_html(html).unwrap();
        assert_eq!(comments.len(), 1);
        assert!(comments[0].is_conditional);
    }

    #[test]
    fn test_detect_directive() {
        let html = r#"<!-- DOCTYPE html -->"#;
        let comments = extract_comments_from_html(html).unwrap();
        assert!(comments[0].is_directive);
    }

    #[test]
    fn test_comment_to_markdown_html_format() {
        let comment = HtmlComment {
            content: "This is a comment".to_string(),
            is_directive: false,
            is_conditional: false,
        };
        let markdown = comment_to_markdown(&comment, true);
        assert!(markdown.contains("<!--"));
        assert!(markdown.contains("-->"));
    }

    #[test]
    fn test_comment_to_markdown_note_format() {
        let comment = HtmlComment {
            content: "This is a comment".to_string(),
            is_directive: false,
            is_conditional: false,
        };
        let markdown = comment_to_markdown(&comment, false);
        assert!(markdown.contains("💬"));
        assert!(markdown.contains("Note"));
    }

    #[test]
    fn test_comment_to_markdown_conditional() {
        let comment = HtmlComment {
            content: "[if IE]IE specific".to_string(),
            is_directive: false,
            is_conditional: true,
        };
        let markdown = comment_to_markdown(&comment, false);
        assert!(markdown.contains("IE Conditional"));
    }

    #[test]
    fn test_remove_comments() {
        let html = r#"<p>Before<!-- comment --></p>After"#;
        let (result, comments) = process_comments_in_html(html, false).unwrap();
        assert_eq!(comments.len(), 1);
        assert!(!result.contains("<!--"));
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
    }

    #[test]
    fn test_preserve_comments() {
        let html = r#"<p>Text<!-- comment --></p>"#;
        let (result, comments) = process_comments_in_html(html, true).unwrap();
        assert_eq!(comments.len(), 1);
        assert!(result.contains("<!--"));
    }

    #[test]
    fn test_comment_summary() {
        let comments = vec![
            HtmlComment {
                content: "Regular comment".to_string(),
                is_directive: false,
                is_conditional: false,
            },
            HtmlComment {
                content: "[if IE]Conditional".to_string(),
                is_directive: false,
                is_conditional: true,
            },
        ];
        let summary = generate_comment_summary(&comments);
        assert!(summary.contains("2 comments"));
        assert!(summary.contains("Conditional"));
        assert!(summary.contains("Regular"));
    }

    #[test]
    fn test_empty_html_no_comments() {
        let html = r#"<p>No comments here</p>"#;
        let comments = extract_comments_from_html(html).unwrap();
        assert_eq!(comments.len(), 0);
    }

    #[test]
    fn test_multiline_comment() {
        let html = r#"<!--
            This is a
            multiline comment
            with lots of content
        -->"#;
        let comments = extract_comments_from_html(html).unwrap();
        assert_eq!(comments.len(), 1);
        assert!(comments[0].content.contains("multiline"));
    }

    #[test]
    fn test_comment_with_dashes() {
        let html = r#"<!-- This comment has - dashes - in it -->"#;
        let comments = extract_comments_from_html(html).unwrap();
        assert_eq!(comments.len(), 1);
        assert!(comments[0].content.contains("dashes"));
    }
}
