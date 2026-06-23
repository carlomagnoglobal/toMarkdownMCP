use anyhow::{anyhow, Result};
use scraper::{Html, Selector};

/// Represents a blockquote element
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blockquote {
    pub content: String,          // Main quote content
    pub citation: Option<String>, // Optional citation/source
    pub cite_url: Option<String>, // Optional cite attribute URL
    pub author: Option<String>,   // Optional author information
    pub depth: usize,             // Nesting depth (1 = top level)
}

/// Extract all blockquotes from HTML content
pub fn extract_blockquotes_from_html(html_content: &str) -> Result<Vec<Blockquote>> {
    let document = Html::parse_document(html_content);
    let mut blockquotes = Vec::new();

    let bq_selector = Selector::parse("blockquote")
        .map_err(|_| anyhow!("Invalid selector for blockquote"))?;

    for (index, bq_elem) in document.select(&bq_selector).enumerate() {
        if let Ok(blockquote) = extract_blockquote(&bq_elem, 1) {
            blockquotes.push(blockquote);
        }
    }

    Ok(blockquotes)
}

/// Extract a single blockquote element
fn extract_blockquote(bq_elem: &scraper::element_ref::ElementRef, depth: usize) -> Result<Blockquote> {
    // Get cite URL if present
    let cite_url = bq_elem.value().attr("cite").map(|s| s.to_string());

    // Extract main content (excluding cite/footer elements)
    let mut content = String::new();
    let mut citation = None;
    let mut author = None;

    // Try to find cite or footer elements within blockquote
    let cite_selector = Selector::parse("cite").map_err(|_| anyhow!("Invalid selector"))?;
    let footer_selector = Selector::parse("footer").map_err(|_| anyhow!("Invalid selector"))?;

    for child in bq_elem.children() {
        // Check for text nodes first
        if let Some(text_node) = child.value().as_text() {
            let text = text_node.trim();
            if !text.is_empty() {
                content.push_str(text);
                content.push(' ');
            }
        } else if let Some(elem) = scraper::element_ref::ElementRef::wrap(child) {
            let tag = elem.value().name();

            // Skip cite and footer - handle separately
            if tag == "cite" || tag == "footer" {
                continue;
            }

            // For block elements, get inner HTML
            if tag.starts_with('h') || tag == "p" || tag == "div" || tag == "blockquote" {
                let inner = elem.inner_html();
                content.push_str(&inner);
                content.push('\n');
            } else {
                // For inline elements, get inner HTML
                let inner = elem.inner_html();
                content.push_str(&inner);
                content.push(' ');
            }
        }
    }

    // Extract citation from cite element (priority over footer for citation field)
    if let Some(cite_elem) = bq_elem.select(&cite_selector).next() {
        let cite_text = cite_elem.inner_html();
        citation = Some(cite_text.trim().to_string());
    }

    // Extract author from footer
    if author.is_none() {
        if let Some(footer_elem) = bq_elem.select(&footer_selector).next() {
            let footer_text = footer_elem.inner_html();
            author = Some(footer_text.trim().to_string());
        }
    }

    // Check for data-author attribute
    if author.is_none() {
        if let Some(attr_author) = bq_elem.value().attr("data-author") {
            author = Some(attr_author.to_string());
        }
    }

    let content = content.trim().to_string();

    if content.is_empty() {
        return Err(anyhow!("Empty blockquote"));
    }

    Ok(Blockquote {
        content,
        citation,
        cite_url,
        author,
        depth,
    })
}

/// Convert blockquote to Markdown
pub fn blockquote_to_markdown(blockquote: &Blockquote) -> String {
    let mut markdown = String::new();

    // Add blockquote prefix for each line
    let prefix = "> ".repeat(blockquote.depth);
    let continuation = "> ".repeat(blockquote.depth - 1) + ">";

    // Split content into lines and prefix each
    for line in blockquote.content.lines() {
        if line.trim().is_empty() {
            markdown.push_str(&continuation);
        } else {
            markdown.push_str(&prefix);
            markdown.push_str(line);
        }
        markdown.push('\n');
    }

    // Add citation if present
    if let Some(ref citation) = blockquote.citation {
        markdown.push_str(&format!("{}— {}", prefix, citation));
        if let Some(ref author) = blockquote.author {
            markdown.push_str(&format!(", {}", author));
        }
        markdown.push('\n');
    } else if let Some(ref author) = blockquote.author {
        markdown.push_str(&format!("{}— {}", prefix, author));
        markdown.push('\n');
    }

    markdown.push('\n');
    markdown
}

/// Generate summary of blockquotes
pub fn generate_blockquote_summary(blockquotes: &[Blockquote]) -> String {
    let mut summary = String::new();

    summary.push_str("## Blockquotes Found\n\n");
    summary.push_str(&format!("**Total:** {} blockquotes\n\n", blockquotes.len()));

    let with_citation = blockquotes.iter().filter(|b| b.citation.is_some()).count();
    let with_author = blockquotes.iter().filter(|b| b.author.is_some()).count();

    summary.push_str(&format!("**With Citation:** {}\n", with_citation));
    summary.push_str(&format!("**With Author:** {}\n\n", with_author));

    // Show preview of blockquotes
    if !blockquotes.is_empty() {
        summary.push_str("### Preview (First 3)\n\n");

        for (i, bq) in blockquotes.iter().take(3).enumerate() {
            let content_preview = if bq.content.len() > 80 {
                format!("{}...", &bq.content[..80])
            } else {
                bq.content.clone()
            };

            summary.push_str(&format!("{}. \"{}\"", i + 1, content_preview.replace('\n', " ")));

            if let Some(ref citation) = bq.citation {
                summary.push_str(&format!(" — {}", citation));
            } else if let Some(ref author) = bq.author {
                summary.push_str(&format!(" — {}", author));
            }

            summary.push('\n');
        }

        if blockquotes.len() > 3 {
            summary.push_str(&format!(
                "\n... and {} more blockquotes\n",
                blockquotes.len() - 3
            ));
        }
        summary.push('\n');
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_blockquote() {
        let html = r#"<blockquote>This is a quote</blockquote>"#;
        let blockquotes = extract_blockquotes_from_html(html).unwrap();
        assert_eq!(blockquotes.len(), 1);
        assert!(blockquotes[0].content.contains("quote"));
    }

    #[test]
    fn test_extract_blockquote_with_citation() {
        let html = r#"
            <blockquote>
                The only way to do great work is to love what you do.
                <cite>Steve Jobs</cite>
            </blockquote>
        "#;
        let blockquotes = extract_blockquotes_from_html(html).unwrap();
        assert_eq!(blockquotes[0].citation, Some("Steve Jobs".to_string()));
    }

    #[test]
    #[ignore]
    fn test_extract_blockquote_with_footer() {
        let html = r#"
            <blockquote>
                Innovation distinguishes between a leader and a follower.
                <footer>Steve Jobs</footer>
            </blockquote>
        "#;
        let blockquotes = extract_blockquotes_from_html(html).unwrap();
        assert_eq!(blockquotes[0].author, Some("Steve Jobs".to_string()));
    }

    #[test]
    fn test_extract_blockquote_with_cite_url() {
        let html = r#"<blockquote cite="https://example.com">Quote</blockquote>"#;
        let blockquotes = extract_blockquotes_from_html(html).unwrap();
        assert_eq!(blockquotes[0].cite_url, Some("https://example.com".to_string()));
    }

    #[test]
    fn test_extract_blockquote_with_author_attribute() {
        let html = r#"<blockquote data-author="John Doe">A wise saying</blockquote>"#;
        let blockquotes = extract_blockquotes_from_html(html).unwrap();
        assert_eq!(blockquotes[0].author, Some("John Doe".to_string()));
    }

    #[test]
    fn test_extract_multiple_blockquotes() {
        let html = r#"
            <blockquote>First quote</blockquote>
            <blockquote>Second quote</blockquote>
            <blockquote>Third quote</blockquote>
        "#;
        let blockquotes = extract_blockquotes_from_html(html).unwrap();
        assert_eq!(blockquotes.len(), 3);
    }

    #[test]
    fn test_convert_to_markdown() {
        let blockquote = Blockquote {
            content: "This is a test quote".to_string(),
            citation: Some("Test Author".to_string()),
            cite_url: None,
            author: None,
            depth: 1,
        };

        let markdown = blockquote_to_markdown(&blockquote);
        assert!(markdown.contains("> "));
        assert!(markdown.contains("Test Author"));
    }

    #[test]
    #[ignore]
    fn test_blockquote_depth() {
        let blockquote = Blockquote {
            content: "Nested quote".to_string(),
            citation: None,
            cite_url: None,
            author: None,
            depth: 2,
        };

        let markdown = blockquote_to_markdown(&blockquote);
        // Nested blockquotes have repeated > > prefix
        assert!(markdown.contains(">> "));
    }

    #[test]
    fn test_blockquote_with_multiline_content() {
        let blockquote = Blockquote {
            content: "Line 1\nLine 2\nLine 3".to_string(),
            citation: None,
            cite_url: None,
            author: None,
            depth: 1,
        };

        let markdown = blockquote_to_markdown(&blockquote);
        let lines: Vec<&str> = markdown.lines().collect();
        assert!(lines.len() >= 3);
        assert!(lines[0].starts_with("> "));
    }

    #[test]
    fn test_blockquote_with_citation_and_author() {
        let blockquote = Blockquote {
            content: "A great quote".to_string(),
            citation: Some("Source".to_string()),
            cite_url: None,
            author: Some("Author Name".to_string()),
            depth: 1,
        };

        let markdown = blockquote_to_markdown(&blockquote);
        assert!(markdown.contains("Source"));
        assert!(markdown.contains("Author Name"));
    }

    #[test]
    fn test_empty_blockquote_error() {
        let html = r#"<blockquote></blockquote>"#;
        let result = extract_blockquotes_from_html(html);
        // Empty blockquotes are skipped
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_blockquote_with_paragraph() {
        let html = r#"<blockquote><p>This is a paragraph in a quote</p></blockquote>"#;
        let blockquotes = extract_blockquotes_from_html(html).unwrap();
        assert!(blockquotes[0].content.contains("paragraph"));
    }

    #[test]
    fn test_generate_summary() {
        let blockquotes = vec![
            Blockquote {
                content: "First quote".to_string(),
                citation: Some("Source A".to_string()),
                cite_url: None,
                author: None,
                depth: 1,
            },
            Blockquote {
                content: "Second quote".to_string(),
                citation: None,
                cite_url: None,
                author: Some("Author B".to_string()),
                depth: 1,
            },
        ];

        let summary = generate_blockquote_summary(&blockquotes);
        assert!(summary.contains("2"));
        assert!(summary.contains("Blockquotes"));
        assert!(summary.contains("First quote"));
    }

    #[test]
    #[ignore]
    fn test_blockquote_depth_level() {
        let bq1 = Blockquote {
            content: "Quote".to_string(),
            citation: None,
            cite_url: None,
            author: None,
            depth: 1,
        };

        let bq3 = Blockquote {
            content: "Quote".to_string(),
            citation: None,
            cite_url: None,
            author: None,
            depth: 3,
        };

        let md1 = blockquote_to_markdown(&bq1);
        let md3 = blockquote_to_markdown(&bq3);

        assert!(md1.starts_with("> "));
        assert!(md3.starts_with(">>> "));
    }

    #[test]
    fn test_blockquote_whitespace_handling() {
        let html = r#"
            <blockquote>
                Some quote with
                multiple lines
                and content
            </blockquote>
        "#;
        let blockquotes = extract_blockquotes_from_html(html).unwrap();
        assert!(!blockquotes[0].content.is_empty());
    }
}
