use anyhow::{anyhow, Result};
use scraper::{Html, Selector, element_ref::ElementRef};
use std::collections::BTreeMap;

/// Represents a heading in the document
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: usize,           // 1-6
    pub text: String,           // Visible text
    pub id: Option<String>,     // HTML id attribute
    pub parent_levels: Vec<usize>, // Ancestor levels for hierarchy
}

impl Heading {
    /// Get indentation string for visualization
    pub fn get_indent(&self) -> String {
        "  ".repeat(self.level - 1)
    }

    /// Get heading symbol for Markdown
    pub fn get_markdown_symbol(&self) -> String {
        "#".repeat(self.level)
    }
}

/// Statistics about document heading structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingStatistics {
    pub total_headings: usize,
    pub levels_count: BTreeMap<usize, usize>,
    pub max_depth: usize,
    pub min_depth: usize,
    pub has_hierarchy_issues: bool,
    pub issues: Vec<String>,
}

/// Extract all headings from HTML content
pub fn extract_headings_from_html(html_content: &str) -> Result<Vec<Heading>> {
    let document = Html::parse_document(html_content);
    let mut headings = Vec::new();

    // Use a more general selector to get all headings in document order
    for element in document.root_element().descendants() {
        if let Some(elem_ref) = scraper::element_ref::ElementRef::wrap(element) {
            let tag_name = elem_ref.value().name();

            // Check if this is a heading tag
            if let Some(level_char) = tag_name.strip_prefix('h') {
                if let Ok(level) = level_char.parse::<usize>() {
                    if (1..=6).contains(&level) {
                        let text = elem_ref.inner_html();
                        let text = text.trim().to_string();

                        // Skip empty headings
                        if text.is_empty() {
                            continue;
                        }

                        let id = elem_ref.value().attr("id").map(|s| s.to_string());

                        headings.push(Heading {
                            level,
                            text,
                            id,
                            parent_levels: Vec::new(),
                        });
                    }
                }
            }
        }
    }

    // Build hierarchy (parent_levels)
    build_hierarchy(&mut headings);

    Ok(headings)
}

/// Build parent hierarchy for headings
fn build_hierarchy(headings: &mut [Heading]) {
    let mut hierarchy_stack: Vec<usize> = Vec::new();

    for heading in headings.iter_mut() {
        // Remove levels >= current from stack (going back up the hierarchy)
        while !hierarchy_stack.is_empty() && *hierarchy_stack.last().unwrap() >= heading.level {
            hierarchy_stack.pop();
        }

        // Current hierarchy path is the stack
        heading.parent_levels = hierarchy_stack.clone();

        // Add current level to stack
        hierarchy_stack.push(heading.level);
    }
}

/// Analyze heading structure for issues
pub fn analyze_heading_structure(headings: &[Heading]) -> HeadingStatistics {
    let mut levels_count: BTreeMap<usize, usize> = BTreeMap::new();
    let mut issues = Vec::new();
    let mut prev_level = 0;
    let mut max_depth = 0;
    let mut min_depth = 6;

    for heading in headings {
        *levels_count.entry(heading.level).or_insert(0) += 1;
        max_depth = max_depth.max(heading.level);
        min_depth = min_depth.min(heading.level);

        // Check for improper jumps (e.g., h1 -> h3)
        if prev_level > 0 && heading.level > prev_level + 1 {
            issues.push(format!(
                "Jump from h{} to h{} (missing h{})",
                prev_level,
                heading.level,
                prev_level + 1
            ));
        }

        prev_level = heading.level;
    }

    // Check if document starts with h1
    if !headings.is_empty() && headings[0].level != 1 {
        issues.push(format!(
            "Document should start with h1, but starts with h{}",
            headings[0].level
        ));
    }

    // Check for multiple h1s
    let h1_count = levels_count.get(&1).copied().unwrap_or(0);
    if h1_count > 1 {
        issues.push(format!("Document has {} h1 tags (should be 1)", h1_count));
    } else if h1_count == 0 && !headings.is_empty() {
        issues.push("Document has no h1 tag".to_string());
    }

    HeadingStatistics {
        total_headings: headings.len(),
        levels_count,
        max_depth,
        min_depth: if min_depth == 6 && headings.is_empty() { 0 } else { min_depth },
        has_hierarchy_issues: !issues.is_empty(),
        issues,
    }
}

/// Generate a visual tree of headings
pub fn generate_heading_tree(headings: &[Heading]) -> String {
    let mut tree = String::new();

    tree.push_str("## Document Heading Structure\n\n");
    tree.push_str("```\n");

    for heading in headings {
        tree.push_str(&heading.get_indent());
        tree.push_str(&format!("├─ H{}: {}\n", heading.level, heading.text));
    }

    tree.push_str("```\n\n");

    tree
}

/// Generate heading outline in Markdown
pub fn generate_heading_outline(headings: &[Heading]) -> String {
    let mut outline = String::new();

    outline.push_str("## Document Outline\n\n");

    for (i, heading) in headings.iter().enumerate() {
        let indent = "  ".repeat(heading.level - 1);
        let number = i + 1;

        if let Some(ref id) = heading.id {
            outline.push_str(&format!(
                "{}{}. [{}](#{})\n",
                indent, number, heading.text, id
            ));
        } else {
            outline.push_str(&format!("{}{}. {}\n", indent, number, heading.text));
        }
    }

    outline.push('\n');

    outline
}

/// Generate heading statistics summary
pub fn generate_heading_summary(stats: &HeadingStatistics) -> String {
    let mut summary = String::new();

    summary.push_str("## Heading Structure Analysis\n\n");

    summary.push_str(&format!("**Total Headings:** {}\n\n", stats.total_headings));

    summary.push_str("### Heading Levels Distribution\n\n");
    summary.push_str("| Level | Count |\n");
    summary.push_str("|-------|-------|\n");

    for level in 1..=6 {
        if let Some(count) = stats.levels_count.get(&level) {
            summary.push_str(&format!("| H{} | {} |\n", level, count));
        }
    }
    summary.push('\n');

    summary.push_str(&format!(
        "**Hierarchy Depth:** {} - {}\n\n",
        stats.min_depth, stats.max_depth
    ));

    if stats.has_hierarchy_issues {
        summary.push_str("### ⚠️ Hierarchy Issues\n\n");
        for issue in &stats.issues {
            summary.push_str(&format!("- {}\n", issue));
        }
        summary.push('\n');
    } else {
        summary.push_str("### ✓ No Hierarchy Issues\n\n");
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_heading() {
        let html = r#"<h1>Main Title</h1>"#;
        let headings = extract_headings_from_html(html).unwrap();
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Main Title");
    }

    #[test]
    fn test_extract_multiple_headings() {
        let html = r#"
            <h1>Title</h1>
            <h2>Subtitle</h2>
            <h3>Section</h3>
        "#;
        let headings = extract_headings_from_html(html).unwrap();
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[2].level, 3);
    }

    #[test]
    fn test_extract_heading_with_id() {
        let html = r#"<h2 id="section-1">Section</h2>"#;
        let headings = extract_headings_from_html(html).unwrap();
        assert_eq!(headings[0].id, Some("section-1".to_string()));
    }

    #[test]
    fn test_skip_empty_headings() {
        let html = r#"<h1></h1><h2>Content</h2>"#;
        let headings = extract_headings_from_html(html).unwrap();
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].text, "Content");
    }

    #[test]
    fn test_all_heading_levels() {
        let html = r#"
            <h1>Level 1</h1>
            <h2>Level 2</h2>
            <h3>Level 3</h3>
            <h4>Level 4</h4>
            <h5>Level 5</h5>
            <h6>Level 6</h6>
        "#;
        let headings = extract_headings_from_html(html).unwrap();
        assert_eq!(headings.len(), 6);
        for (i, heading) in headings.iter().enumerate() {
            assert_eq!(heading.level, i + 1);
        }
    }

    #[test]
    fn test_hierarchy_building() {
        let html = r#"
            <h1>Title</h1>
            <h2>Section 1</h2>
            <h3>Subsection</h3>
            <h2>Section 2</h2>
        "#;
        let headings = extract_headings_from_html(html).unwrap();
        assert_eq!(headings[0].parent_levels, vec![] as Vec<usize>);
        assert_eq!(headings[1].parent_levels, vec![1]);
        assert_eq!(headings[2].parent_levels, vec![1, 2]);
        assert_eq!(headings[3].parent_levels, vec![1]);
    }

    #[test]
    fn test_analyze_good_structure() {
        let html = r#"
            <h1>Title</h1>
            <h2>Section 1</h2>
            <h3>Subsection</h3>
            <h2>Section 2</h2>
        "#;
        let headings = extract_headings_from_html(html).unwrap();
        let stats = analyze_heading_structure(&headings);
        assert!(!stats.has_hierarchy_issues);
        assert_eq!(stats.total_headings, 4);
    }

    #[test]
    fn test_analyze_jump_issue() {
        let html = r#"
            <h1>Title</h1>
            <h3>Section</h3>
        "#;
        let headings = extract_headings_from_html(html).unwrap();
        let stats = analyze_heading_structure(&headings);
        assert!(stats.has_hierarchy_issues);
        assert!(stats.issues[0].contains("Jump from h1 to h3"));
    }

    #[test]
    fn test_analyze_no_h1() {
        let html = r#"
            <h2>Section</h2>
            <h3>Subsection</h3>
        "#;
        let headings = extract_headings_from_html(html).unwrap();
        let stats = analyze_heading_structure(&headings);
        assert!(stats.has_hierarchy_issues);
    }

    #[test]
    fn test_analyze_multiple_h1s() {
        let html = r#"
            <h1>Title 1</h1>
            <h1>Title 2</h1>
        "#;
        let headings = extract_headings_from_html(html).unwrap();
        let stats = analyze_heading_structure(&headings);
        assert!(stats.has_hierarchy_issues);
        assert!(stats.issues.iter().any(|i| i.contains("h1 tags")));
    }

    #[test]
    fn test_heading_indent() {
        let heading = Heading {
            level: 3,
            text: "Section".to_string(),
            id: None,
            parent_levels: vec![1, 2],
        };
        assert_eq!(heading.get_indent(), "    "); // 2 levels = 4 spaces
    }

    #[test]
    fn test_heading_markdown_symbol() {
        for level in 1..=6 {
            let heading = Heading {
                level,
                text: "Test".to_string(),
                id: None,
                parent_levels: Vec::new(),
            };
            let symbol = heading.get_markdown_symbol();
            assert_eq!(symbol, "#".repeat(level));
        }
    }

    #[test]
    fn test_generate_heading_tree() {
        let headings = vec![
            Heading {
                level: 1,
                text: "Title".to_string(),
                id: None,
                parent_levels: vec![],
            },
            Heading {
                level: 2,
                text: "Section".to_string(),
                id: None,
                parent_levels: vec![1],
            },
        ];
        let tree = generate_heading_tree(&headings);
        assert!(tree.contains("H1: Title"));
        assert!(tree.contains("H2: Section"));
    }

    #[test]
    fn test_generate_heading_outline() {
        let headings = vec![
            Heading {
                level: 1,
                text: "Title".to_string(),
                id: Some("title".to_string()),
                parent_levels: vec![],
            },
            Heading {
                level: 2,
                text: "Section".to_string(),
                id: Some("section".to_string()),
                parent_levels: vec![1],
            },
        ];
        let outline = generate_heading_outline(&headings);
        assert!(outline.contains("[Title](#title)"));
        assert!(outline.contains("[Section](#section)"));
    }

    #[test]
    fn test_generate_heading_summary() {
        let stats = HeadingStatistics {
            total_headings: 4,
            levels_count: {
                let mut m = BTreeMap::new();
                m.insert(1, 1);
                m.insert(2, 2);
                m.insert(3, 1);
                m
            },
            max_depth: 3,
            min_depth: 1,
            has_hierarchy_issues: false,
            issues: vec![],
        };
        let summary = generate_heading_summary(&stats);
        assert!(summary.contains("4"));
        assert!(summary.contains("No Hierarchy Issues"));
    }

    #[test]
    fn test_headings_with_formatting() {
        let html = r#"<h2><strong>Bold</strong> Heading</h2>"#;
        let headings = extract_headings_from_html(html).unwrap();
        assert_eq!(headings.len(), 1);
        assert!(headings[0].text.contains("Bold"));
    }

    #[test]
    fn test_empty_document() {
        let html = r#"<p>No headings here</p>"#;
        let headings = extract_headings_from_html(html).unwrap();
        assert_eq!(headings.len(), 0);
    }
}
