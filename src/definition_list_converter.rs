use anyhow::{anyhow, Result};
use scraper::{Html, Selector};

/// Represents a definition in a definition list
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub term: String,           // The term being defined
    pub descriptions: Vec<String>, // One or more descriptions
}

/// Represents a definition list
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionList {
    pub definitions: Vec<Definition>,
    pub compact: bool, // Whether list is compact (single-line style)
}

impl DefinitionList {
    /// Check if list should be compact format
    fn is_compact(&self) -> bool {
        self.definitions.iter().all(|d| d.descriptions.len() == 1 && d.descriptions[0].len() < 100)
    }
}

/// Extract all definition lists from HTML content
pub fn extract_definition_lists_from_html(html_content: &str) -> Result<Vec<DefinitionList>> {
    let document = Html::parse_document(html_content);
    let mut lists = Vec::new();

    let dl_selector = Selector::parse("dl").map_err(|_| anyhow!("Invalid selector for dl"))?;

    for dl_elem in document.select(&dl_selector) {
        if let Ok(list) = parse_definition_list(&dl_elem) {
            lists.push(list);
        }
    }

    Ok(lists)
}

/// Parse a single definition list element
pub fn parse_definition_list(dl_elem: &scraper::element_ref::ElementRef) -> Result<DefinitionList> {
    let _dt_selector = Selector::parse("dt").map_err(|_| anyhow!("Invalid selector"))?;
    let _dd_selector = Selector::parse("dd").map_err(|_| anyhow!("Invalid selector"))?;

    let mut definitions = Vec::new();
    let mut current_terms: Vec<String> = Vec::new();
    let mut current_descriptions: Vec<String> = Vec::new();

    // Process all direct children to maintain order
    for child in dl_elem.children() {
        if let Some(elem_ref) = scraper::element_ref::ElementRef::wrap(child) {
            let tag_name = elem_ref.value().name();

            match tag_name {
                "dt" => {
                    // If we have previous terms with descriptions, save them
                    if !current_terms.is_empty() && !current_descriptions.is_empty() {
                        for term in current_terms.drain(..) {
                            definitions.push(Definition {
                                term,
                                descriptions: current_descriptions.clone(),
                            });
                        }
                        current_descriptions.clear();
                    }

                    // Extract new term
                    let term_text = elem_ref.inner_html();
                    let term_text = term_text.trim().to_string();

                    if !term_text.is_empty() {
                        current_terms.push(term_text);
                    }
                }
                "dd" => {
                    // Extract description and add to current descriptions
                    let desc_text = elem_ref.inner_html();
                    let desc_text = desc_text.trim().to_string();

                    if !desc_text.is_empty() {
                        current_descriptions.push(desc_text);
                    }
                }
                _ => {} // Ignore other elements
            }
        }
    }

    // Handle remaining terms/descriptions
    if !current_terms.is_empty() && !current_descriptions.is_empty() {
        for term in current_terms {
            definitions.push(Definition {
                term,
                descriptions: current_descriptions.clone(),
            });
        }
    }

    if definitions.is_empty() {
        return Err(anyhow!("Empty definition list"));
    }

    let mut list = DefinitionList {
        definitions,
        compact: false,
    };

    list.compact = list.is_compact();

    Ok(list)
}

/// Convert definition list to Markdown
pub fn definition_list_to_markdown(list: &DefinitionList) -> String {
    let mut markdown = String::new();

    if list.compact {
        // Compact format: term : description
        for definition in &list.definitions {
            markdown.push_str(&format!(
                "**{}** : {}\n\n",
                definition.term.replace('|', "\\|"),
                definition.descriptions.join(" ")
            ));
        }
    } else {
        // Expanded format with descriptions on separate lines
        for (i, definition) in list.definitions.iter().enumerate() {
            markdown.push_str(&format!("**{}**\n", definition.term.replace('|', "\\|")));

            for desc in &definition.descriptions {
                markdown.push_str(&format!(":   {}\n", desc.replace('|', "\\|")));
            }

            if i < list.definitions.len() - 1 {
                markdown.push('\n');
            }
        }
        markdown.push('\n');
    }

    markdown
}

/// Generate table format for definition list (alternative format)
pub fn definition_list_to_table(list: &DefinitionList) -> String {
    let mut markdown = String::new();

    markdown.push_str("| Term | Definition |\n");
    markdown.push_str("|------|------------|\n");

    for definition in &list.definitions {
        let descriptions = definition.descriptions.join("; ");
        markdown.push_str(&format!(
            "| {} | {} |\n",
            definition.term.replace('|', "\\|"),
            descriptions.replace('|', "\\|")
        ));
    }

    markdown.push('\n');
    markdown
}

/// Convert all definition lists in HTML to Markdown
pub fn convert_definition_lists_in_html(html_content: &str) -> Result<String> {
    let lists = extract_definition_lists_from_html(html_content)?;

    if lists.is_empty() {
        return Ok(html_content.to_string());
    }

    let mut result = html_content.to_string();

    // Replace definition lists with Markdown
    for _list in &lists {
        // Find and replace each <dl>...</dl> with Markdown version
        if let Some(dl_start) = result.find("<dl") {
            if let Some(dl_end) = result[dl_start..].find("</dl>") {
                let _actual_end = dl_start + dl_end + 5; // +5 for "</dl>"
                // For now, we just remove the tags and let the content flow
                // A more sophisticated implementation would parse and reconstruct
                result.drain(dl_start..dl_start + result[dl_start..].find('>').unwrap_or(0) + 1);
                if let Some(end) = result[dl_start..].find("</dl>") {
                    result.drain(dl_start + end..dl_start + end + 5);
                }
            }
        }
    }

    Ok(result)
}

/// Generate summary of definition lists
pub fn generate_definition_list_summary(lists: &[DefinitionList]) -> String {
    let mut summary = String::new();

    summary.push_str("## Definition Lists Found\n\n");
    summary.push_str(&format!("**Total:** {} definition lists\n\n", lists.len()));

    let total_definitions: usize = lists.iter().map(|l| l.definitions.len()).sum();
    let total_descriptions: usize = lists
        .iter()
        .flat_map(|l| &l.definitions)
        .map(|d| d.descriptions.len())
        .sum();

    summary.push_str(&format!("**Definitions:** {}\n", total_definitions));
    summary.push_str(&format!("**Descriptions:** {}\n\n", total_descriptions));

    // Show first few definitions as preview
    if !lists.is_empty() {
        summary.push_str("### Preview (First List)\n\n");
        let preview_list = &lists[0];
        let preview_count = preview_list.definitions.len().min(5);

        for (i, def) in preview_list.definitions.iter().take(preview_count).enumerate() {
            let desc_preview = if def.descriptions[0].len() > 60 {
                format!("{}...", &def.descriptions[0][..60])
            } else {
                def.descriptions[0].clone()
            };

            summary.push_str(&format!(
                "{}. **{}** - {}\n",
                i + 1,
                def.term.replace('|', "\\|"),
                desc_preview.replace('|', "\\|")
            ));
        }

        if preview_list.definitions.len() > 5 {
            summary.push_str(&format!(
                "\n... and {} more definitions\n",
                preview_list.definitions.len() - 5
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
    fn test_extract_simple_definition_list() {
        let html = r#"
            <dl>
                <dt>HTML</dt>
                <dd>HyperText Markup Language</dd>
                <dt>CSS</dt>
                <dd>Cascading Style Sheets</dd>
            </dl>
        "#;
        let lists = extract_definition_lists_from_html(html).unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].definitions.len(), 2);
        assert_eq!(lists[0].definitions[0].term, "HTML");
    }

    #[test]
    fn test_extract_with_multiple_descriptions() {
        let html = r#"
            <dl>
                <dt>Term</dt>
                <dd>First definition</dd>
                <dd>Second definition</dd>
            </dl>
        "#;
        let lists = extract_definition_lists_from_html(html).unwrap();
        assert_eq!(lists[0].definitions[0].descriptions.len(), 2);
        assert_eq!(lists[0].definitions[0].descriptions[0], "First definition");
        assert_eq!(lists[0].definitions[0].descriptions[1], "Second definition");
    }

    #[test]
    fn test_extract_multiple_lists() {
        let html = r#"
            <dl>
                <dt>Term 1</dt>
                <dd>Definition 1</dd>
            </dl>
            <dl>
                <dt>Term 2</dt>
                <dd>Definition 2</dd>
            </dl>
        "#;
        let lists = extract_definition_lists_from_html(html).unwrap();
        assert_eq!(lists.len(), 2);
    }

    #[test]
    fn test_convert_to_markdown() {
        let list = DefinitionList {
            definitions: vec![
                Definition {
                    term: "HTML".to_string(),
                    descriptions: vec!["HyperText Markup Language".to_string()],
                },
                Definition {
                    term: "CSS".to_string(),
                    descriptions: vec!["Cascading Style Sheets".to_string()],
                },
            ],
            compact: true,
        };

        let markdown = definition_list_to_markdown(&list);
        assert!(markdown.contains("**HTML**"));
        assert!(markdown.contains("**CSS**"));
        assert!(markdown.contains("HyperText Markup Language"));
    }

    #[test]
    fn test_convert_to_table() {
        let list = DefinitionList {
            definitions: vec![Definition {
                term: "HTML".to_string(),
                descriptions: vec!["HyperText Markup Language".to_string()],
            }],
            compact: false,
        };

        let table = definition_list_to_table(&list);
        assert!(table.contains("| Term | Definition |"));
        assert!(table.contains("| HTML |"));
    }

    #[test]
    fn test_compact_detection() {
        let compact_list = DefinitionList {
            definitions: vec![Definition {
                term: "Short".to_string(),
                descriptions: vec!["Brief".to_string()],
            }],
            compact: false,
        };

        assert!(compact_list.is_compact());
    }

    #[test]
    fn test_non_compact_detection() {
        let long_desc = "a".repeat(150);
        let list = DefinitionList {
            definitions: vec![Definition {
                term: "Term".to_string(),
                descriptions: vec![long_desc],
            }],
            compact: false,
        };

        assert!(!list.is_compact());
    }

    #[test]
    fn test_escape_pipe_characters() {
        let list = DefinitionList {
            definitions: vec![Definition {
                term: "Term | With Pipe".to_string(),
                descriptions: vec!["Definition | Also Pipe".to_string()],
            }],
            compact: true,
        };

        let markdown = definition_list_to_markdown(&list);
        assert!(markdown.contains("\\|"));
    }

    #[test]
    fn test_multiple_terms_same_definition() {
        let html = r#"
            <dl>
                <dt>Python</dt>
                <dt>Py</dt>
                <dd>A programming language</dd>
            </dl>
        "#;
        let lists = extract_definition_lists_from_html(html).unwrap();
        assert_eq!(lists[0].definitions.len(), 2);
    }

    #[test]
    fn test_empty_definition_list_skipped() {
        let html = r#"<dl></dl>"#;
        let result = extract_definition_lists_from_html(html);
        // Empty definition lists are skipped, so result is Ok(empty vec)
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_definition_with_formatting() {
        let html = r#"
            <dl>
                <dt><strong>Bold Term</strong></dt>
                <dd><em>Italic definition</em></dd>
            </dl>
        "#;
        let lists = extract_definition_lists_from_html(html).unwrap();
        assert!(lists[0].definitions[0].term.contains("Bold Term"));
        assert!(lists[0].definitions[0].descriptions[0].contains("Italic definition"));
    }

    #[test]
    fn test_generate_summary() {
        let lists = vec![DefinitionList {
            definitions: vec![
                Definition {
                    term: "HTML".to_string(),
                    descriptions: vec!["HyperText Markup Language".to_string()],
                },
                Definition {
                    term: "CSS".to_string(),
                    descriptions: vec!["Cascading Style Sheets".to_string()],
                },
            ],
            compact: true,
        }];

        let summary = generate_definition_list_summary(&lists);
        assert!(summary.contains("1"));
        assert!(summary.contains("Definition Lists"));
    }

    #[test]
    fn test_nested_html_in_definitions() {
        let html = r#"
            <dl>
                <dt>Term</dt>
                <dd>Definition with <a href="https://example.com">link</a></dd>
            </dl>
        "#;
        let lists = extract_definition_lists_from_html(html).unwrap();
        assert!(lists[0].definitions[0].descriptions[0].contains("link"));
    }
}
