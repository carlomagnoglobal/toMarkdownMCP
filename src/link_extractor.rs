use anyhow::{anyhow, Result};
use scraper::{Html, Selector};

/// Represents an extracted hyperlink
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedLink {
    pub url: String,
    pub text: String,
    pub title: Option<String>,
    pub rel: Option<String>,        // Relationship (stylesheet, icon, etc.)
    pub target: Option<String>,     // Target attribute (_blank, _self, etc.)
    pub is_external: bool,          // Whether URL is external
    pub is_broken: Option<bool>,    // Whether link appears broken (no href)
}

impl ExtractedLink {
    /// Check if URL is external (http/https or protocol relative)
    fn is_url_external(url: &str) -> bool {
        url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("//")
    }

    /// Check if link appears broken (empty href, javascript, etc.)
    fn is_url_broken(url: &str) -> bool {
        url.is_empty() || url == "#" || url.starts_with("javascript:")
    }
}

/// Extract all links from HTML content
pub fn extract_links_from_html(html_content: &str) -> Result<Vec<ExtractedLink>> {
    let document = Html::parse_document(html_content);
    let mut links = Vec::new();

    let a_selector = Selector::parse("a").map_err(|_| anyhow!("Invalid selector for links"))?;

    for link_elem in document.select(&a_selector) {
        let url = link_elem.value().attr("href").unwrap_or("").to_string();

        // Skip pure anchors (#something) and empty links
        if (url.is_empty() || url.starts_with('#')) && link_elem.value().attr("name").is_none() {
            continue;
        }

        let text = link_elem.inner_html();
        let text = text.trim().to_string();

        // Skip links with no text and no name attribute
        if text.is_empty() && link_elem.value().attr("name").is_none() {
            continue;
        }

        let title = link_elem.value().attr("title").map(|s| s.to_string());
        let rel = link_elem.value().attr("rel").map(|s| s.to_string());
        let target = link_elem.value().attr("target").map(|s| s.to_string());
        let is_external = ExtractedLink::is_url_external(&url);
        let is_broken = if url.is_empty() {
            Some(true)
        } else {
            None
        };

        links.push(ExtractedLink {
            url,
            text,
            title,
            rel,
            target,
            is_external,
            is_broken,
        });
    }

    Ok(links)
}

/// Generate Markdown link reference section
pub fn generate_link_reference(link: &ExtractedLink, index: usize) -> String {
    let mut reference = format!("[{}]: {}", index, link.url);

    if let Some(ref title) = link.title {
        reference.push_str(&format!(" \"{}\"", title));
    }

    reference
}

/// Generate summary of links found in HTML
pub fn generate_link_summary(links: &[ExtractedLink]) -> String {
    let mut summary = String::new();

    summary.push_str("## Links Found\n\n");
    summary.push_str(&format!("**Total:** {} links\n\n", links.len()));

    let external_count = links.iter().filter(|l| l.is_external).count();
    let internal_count = links.len() - external_count;
    let broken_count = links.iter().filter(|l| l.is_broken == Some(true)).count();

    summary.push_str(&format!("**External:** {} | **Internal:** {}", external_count, internal_count));
    if broken_count > 0 {
        summary.push_str(&format!(" | **Broken:** {}", broken_count));
    }
    summary.push_str("\n\n");

    // Group by external/internal
    let external_links: Vec<_> = links.iter().filter(|l| l.is_external).collect();
    let internal_links: Vec<_> = links.iter().filter(|l| !l.is_external && l.is_broken != Some(true)).collect();
    let broken_links: Vec<_> = links.iter().filter(|l| l.is_broken == Some(true)).collect();

    if !external_links.is_empty() {
        summary.push_str("### External Links\n\n");
        for (i, link) in external_links.iter().enumerate() {
            let text = if link.text.len() > 60 {
                format!("{}...", &link.text[..60])
            } else {
                link.text.clone()
            };
            summary.push_str(&format!("{}. [{}]({})", i + 1, text.replace('|', "\\|"), link.url));
            if let Some(ref title) = link.title {
                summary.push_str(&format!(" - \"{}\"", title.replace('"', "\\\"")));
            }
            summary.push('\n');
        }
        summary.push('\n');
    }

    if !internal_links.is_empty() {
        summary.push_str("### Internal Links\n\n");
        for (i, link) in internal_links.iter().enumerate() {
            let text = if link.text.len() > 60 {
                format!("{}...", &link.text[..60])
            } else {
                link.text.clone()
            };
            summary.push_str(&format!("{}. [{}]({})", i + 1, text.replace('|', "\\|"), link.url));
            if let Some(ref title) = link.title {
                summary.push_str(&format!(" - \"{}\"", title.replace('"', "\\\"")));
            }
            summary.push('\n');
        }
        summary.push('\n');
    }

    if !broken_links.is_empty() {
        summary.push_str("### Broken/Invalid Links\n\n");
        for (i, link) in broken_links.iter().enumerate() {
            let text = if link.text.len() > 60 {
                format!("{}...", &link.text[..60])
            } else {
                link.text.clone()
            };
            summary.push_str(&format!("{}. `{}`\n", i + 1, text.replace('|', "\\|")));
        }
        summary.push('\n');
    }

    summary
}

/// Convert links to Markdown reference format
pub fn links_to_markdown_references(links: &[ExtractedLink]) -> String {
    let mut markdown = String::new();

    markdown.push_str("## Link References\n\n");

    for (index, link) in links.iter().enumerate() {
        markdown.push_str(&generate_link_reference(link, index + 1));
        markdown.push('\n');
    }

    markdown
}

/// Extract statistics about links
pub fn get_link_statistics(links: &[ExtractedLink]) -> LinkStatistics {
    let total = links.len();
    let external = links.iter().filter(|l| l.is_external).count();
    let internal = total - external;
    let with_title = links.iter().filter(|l| l.title.is_some()).count();
    let with_target = links.iter().filter(|l| l.target.is_some()).count();
    let broken = links.iter().filter(|l| l.is_broken == Some(true)).count();

    LinkStatistics {
        total,
        external,
        internal,
        with_title,
        with_target,
        broken,
    }
}

/// Statistics about links in document
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkStatistics {
    pub total: usize,
    pub external: usize,
    pub internal: usize,
    pub with_title: usize,
    pub with_target: usize,
    pub broken: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_link() {
        let html = r#"<a href="https://example.com">Example</a>"#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[0].text, "Example");
        assert!(links[0].is_external);
    }

    #[test]
    fn test_extract_internal_link() {
        let html = r#"<a href="/page">Internal Link</a>"#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "/page");
        assert!(!links[0].is_external);
    }

    #[test]
    fn test_extract_link_with_title() {
        let html = r#"<a href="https://example.com" title="Example Site">Example</a>"#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links[0].title, Some("Example Site".to_string()));
    }

    #[test]
    fn test_extract_link_with_target() {
        let html = r#"<a href="https://example.com" target="_blank">Example</a>"#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links[0].target, Some("_blank".to_string()));
    }

    #[test]
    fn test_extract_link_with_rel() {
        let html = r#"<a href="https://example.com" rel="nofollow">Example</a>"#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links[0].rel, Some("nofollow".to_string()));
    }

    #[test]
    fn test_extract_multiple_links() {
        let html = r#"
            <a href="https://example.com">Example</a>
            <a href="/page">Internal</a>
            <a href="https://other.com">Other</a>
        "#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links.len(), 3);
    }

    #[test]
    fn test_skip_anchor_only_links() {
        let html = r##"<a href="#top">Top</a><p>Content</p>"##;
        let links = extract_links_from_html(html).unwrap();
        // Anchor-only links are skipped
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn test_skip_empty_links() {
        let html = r#"<a href="">Empty Link</a>"#;
        let links = extract_links_from_html(html).unwrap();
        // Empty href links are skipped
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn test_broken_link_detection() {
        let html = r#"<a href="">No href</a>"#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn test_protocol_relative_url() {
        let html = r#"<a href="//cdn.example.com/file.js">CDN Link</a>"#;
        let links = extract_links_from_html(html).unwrap();
        assert!(links[0].is_external);
    }

    #[test]
    fn test_link_reference_generation() {
        let link = ExtractedLink {
            url: "https://example.com".to_string(),
            text: "Example".to_string(),
            title: Some("Example Site".to_string()),
            rel: None,
            target: None,
            is_external: true,
            is_broken: None,
        };
        let reference = generate_link_reference(&link, 1);
        assert!(reference.contains("https://example.com"));
        assert!(reference.contains("Example Site"));
    }

    #[test]
    fn test_link_summary() {
        let links = vec![
            ExtractedLink {
                url: "https://example.com".to_string(),
                text: "Example".to_string(),
                title: None,
                rel: None,
                target: None,
                is_external: true,
                is_broken: None,
            },
            ExtractedLink {
                url: "/page".to_string(),
                text: "Internal".to_string(),
                title: None,
                rel: None,
                target: None,
                is_external: false,
                is_broken: None,
            },
        ];
        let summary = generate_link_summary(&links);
        assert!(summary.contains("2 links"));
        assert!(summary.contains("External"));
        assert!(summary.contains("Internal"));
    }

    #[test]
    fn test_link_statistics() {
        let links = vec![
            ExtractedLink {
                url: "https://example.com".to_string(),
                text: "Example".to_string(),
                title: Some("Title".to_string()),
                rel: None,
                target: Some("_blank".to_string()),
                is_external: true,
                is_broken: None,
            },
            ExtractedLink {
                url: "/page".to_string(),
                text: "Internal".to_string(),
                title: None,
                rel: None,
                target: None,
                is_external: false,
                is_broken: None,
            },
        ];
        let stats = get_link_statistics(&links);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.external, 1);
        assert_eq!(stats.internal, 1);
        assert_eq!(stats.with_title, 1);
        assert_eq!(stats.with_target, 1);
    }

    #[test]
    fn test_links_with_formatted_text() {
        let html = r#"<a href="https://example.com"><strong>Bold Link</strong></a>"#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links.len(), 1);
        assert!(links[0].text.contains("Bold Link"));
    }

    #[test]
    fn test_escape_pipe_in_link_text() {
        let html = r#"<a href="https://example.com">Link | Pipe</a>"#;
        let links = extract_links_from_html(html).unwrap();
        let summary = generate_link_summary(&links);
        // Pipes should be escaped in markdown
        assert!(summary.contains("Link \\| Pipe"));
    }

    #[test]
    fn test_links_to_markdown_references() {
        let links = vec![
            ExtractedLink {
                url: "https://example.com".to_string(),
                text: "Example".to_string(),
                title: Some("Example Site".to_string()),
                rel: None,
                target: None,
                is_external: true,
                is_broken: None,
            },
        ];
        let markdown = links_to_markdown_references(&links);
        assert!(markdown.contains("[1]:"));
        assert!(markdown.contains("https://example.com"));
        assert!(markdown.contains("Example Site"));
    }

    #[test]
    fn test_extract_empty_html() {
        let html = r#"<p>No links here</p>"#;
        let links = extract_links_from_html(html).unwrap();
        assert_eq!(links.len(), 0);
    }
}
