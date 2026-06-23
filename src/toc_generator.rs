use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// Represents a heading in the document
#[derive(Debug, Clone)]
pub struct Heading {
    pub level: usize,      // 1-6 for H1-H6
    pub text: String,      // Raw heading text
    pub anchor: String,    // Generated anchor link
}

/// Generate table of contents from Markdown content
pub fn generate_toc(markdown_content: &str, max_level: usize) -> Result<Vec<Heading>> {
    if max_level < 1 || max_level > 6 {
        return Err(anyhow!("TOC max_level must be between 1 and 6"));
    }

    let mut headings = Vec::new();
    let mut anchor_count: HashMap<String, usize> = HashMap::new();

    for line in markdown_content.lines() {
        // Skip YAML frontmatter
        if line.starts_with("---") {
            continue;
        }

        let trimmed = line.trim();

        // Check for ATX-style headings (# Heading)
        if trimmed.starts_with('#') {
            if let Some((level, text)) = parse_atx_heading(trimmed) {
                if level <= max_level {
                    let anchor = generate_anchor(&text, &mut anchor_count);
                    headings.push(Heading {
                        level,
                        text: text.to_string(),
                        anchor,
                    });
                }
            }
        }
    }

    Ok(headings)
}

/// Parse ATX-style heading (# Heading) and return (level, text)
fn parse_atx_heading(line: &str) -> Option<(usize, &str)> {
    let mut hash_count = 0;

    // Count leading hashes
    for c in line.chars() {
        if c == '#' {
            hash_count += 1;
        } else {
            break;
        }
    }

    if hash_count == 0 || hash_count > 6 {
        return None;
    }

    // Get text after hashes and spaces
    let rest = &line[hash_count..];
    let text = rest.trim_start().trim_end_matches('#').trim();

    if text.is_empty() {
        return None;
    }

    Some((hash_count, text))
}

/// Generate anchor from heading text
fn generate_anchor(text: &str, anchor_count: &mut HashMap<String, usize>) -> String {
    // Convert to lowercase and replace spaces with hyphens
    let mut anchor = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                // Skip other characters
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join("-");

    // Remove multiple consecutive hyphens
    while anchor.contains("--") {
        anchor = anchor.replace("--", "-");
    }

    // Remove leading/trailing hyphens
    anchor = anchor.trim_matches('-').to_string();

    // Handle duplicates by adding a number suffix
    let count = anchor_count.entry(anchor.clone()).or_insert(0);
    if *count > 0 {
        anchor.push('-');
        anchor.push_str(&count.to_string());
    }
    *count += 1;

    anchor
}

/// Format TOC as Markdown list
pub fn format_toc(headings: &[Heading], title: &str) -> String {
    if headings.is_empty() {
        return String::new();
    }

    let mut toc = String::new();
    toc.push_str("## ");
    toc.push_str(title);
    toc.push_str("\n\n");

    let mut prev_level = 0;

    for heading in headings {
        let indent = "  ".repeat(heading.level.saturating_sub(2));

        // Add proper list formatting
        if heading.level > prev_level && prev_level > 0 {
            // Increasing level
        } else if heading.level < prev_level {
            // Decreasing level - may need to close list items
        }

        toc.push_str(&indent);
        toc.push_str("- [");
        toc.push_str(&heading.text);
        toc.push_str("](#");
        toc.push_str(&heading.anchor);
        toc.push_str(")\n");

        prev_level = heading.level;
    }

    toc.push_str("\n");
    toc
}

/// Insert TOC into Markdown content
pub fn insert_toc(markdown_content: &str, toc_content: &str) -> String {
    let mut result = String::new();

    // Find where to insert TOC (after title/metadata if present)
    let lines: Vec<&str> = markdown_content.lines().collect();
    let mut insert_pos = 0;

    // Skip YAML frontmatter
    let mut in_frontmatter = false;

    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "---" {
            if in_frontmatter {
                // End of frontmatter
                insert_pos = i + 1;
                break;
            } else {
                // Start of frontmatter
                in_frontmatter = true;
            }
        }
    }

    // If no frontmatter, check if there's a title (# heading)
    if !in_frontmatter {
        for (i, line) in lines.iter().enumerate() {
            if line.trim().starts_with("# ") {
                insert_pos = i + 1;
                break;
            }
        }
    }

    // Build result
    for (i, line) in lines.iter().enumerate() {
        if i == insert_pos {
            result.push_str(toc_content);
        }
        result.push_str(line);
        result.push('\n');
    }

    // If TOC hasn't been inserted yet, prepend it
    if insert_pos == 0 {
        result = format!("{}{}", toc_content, markdown_content);
    }

    result.trim_end().to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_atx_heading() {
        assert_eq!(parse_atx_heading("# Main Title"), Some((1, "Main Title")));
        assert_eq!(parse_atx_heading("## Subsection"), Some((2, "Subsection")));
        assert_eq!(parse_atx_heading("### Sub-subsection"), Some((3, "Sub-subsection")));
        assert_eq!(parse_atx_heading("#### Level 4"), Some((4, "Level 4")));
        assert_eq!(parse_atx_heading("Regular text"), None);
        assert_eq!(parse_atx_heading("#"), None);
    }

    #[test]
    fn test_generate_anchor() {
        let mut counts = HashMap::new();
        assert_eq!(generate_anchor("Simple Heading", &mut counts), "simple-heading");
        assert_eq!(generate_anchor("Heading With Numbers 123", &mut counts), "heading-with-numbers-123");
        assert_eq!(generate_anchor("Special!@#$%Characters", &mut counts), "special-characters");
        assert_eq!(generate_anchor("Multiple---Hyphens", &mut counts), "multiple-hyphens");
    }

    #[test]
    fn test_generate_anchor_duplicates() {
        let mut counts = HashMap::new();
        let anchor1 = generate_anchor("Title", &mut counts);
        let anchor2 = generate_anchor("Title", &mut counts);

        assert_eq!(anchor1, "title");
        assert_eq!(anchor2, "title-1");
    }

    #[test]
    fn test_generate_toc() {
        let markdown = r#"# Main Title

Some content.

## Section 1
Content here.

### Subsection 1.1
More content.

## Section 2
Final section.
"#;

        let headings = generate_toc(markdown, 6).unwrap();
        assert_eq!(headings.len(), 4);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Main Title");
        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[2].level, 3);
    }

    #[test]
    fn test_generate_toc_with_max_level() {
        let markdown = r#"# Main
## Section
### Subsection
#### Deep
"#;

        let headings = generate_toc(markdown, 2).unwrap();
        assert_eq!(headings.len(), 2); // Only H1 and H2
    }

    #[test]
    fn test_format_toc() {
        let headings = vec![
            Heading { level: 1, text: "Main".to_string(), anchor: "main".to_string() },
            Heading { level: 2, text: "Section".to_string(), anchor: "section".to_string() },
            Heading { level: 3, text: "Subsection".to_string(), anchor: "subsection".to_string() },
        ];

        let toc = format_toc(&headings, "Table of Contents");
        assert!(toc.contains("## Table of Contents"));
        assert!(toc.contains("[Main](#main)"));
        assert!(toc.contains("[Section](#section)"));
        assert!(toc.contains("[Subsection](#subsection)"));
    }
}
