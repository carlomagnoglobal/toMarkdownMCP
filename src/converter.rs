/// Convert text content to Markdown format
pub fn convert_to_markdown(content: &str, language: Option<&str>, title: Option<&str>) -> String {
    let mut markdown = String::new();

    // Add title if provided
    if let Some(title) = title {
        markdown.push_str(&format!("# {}\n\n", title));
    }

    // If language is specified and not empty, treat as code block
    if let Some(lang) = language {
        if !lang.is_empty() {
            markdown.push_str(&format!("```{}\n", lang));
            markdown.push_str(content);
            if !content.ends_with('\n') {
                markdown.push('\n');
            }
            markdown.push_str("```\n");
        } else {
            // Plain text - wrap in code block without language
            markdown.push_str("```\n");
            markdown.push_str(content);
            if !content.ends_with('\n') {
                markdown.push('\n');
            }
            markdown.push_str("```\n");
        }
    } else {
        // No language specified - treat as plain text in code block
        markdown.push_str("```\n");
        markdown.push_str(content);
        if !content.ends_with('\n') {
            markdown.push('\n');
        }
        markdown.push_str("```\n");
    }

    markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_code_with_language() {
        let code = "fn main() {\n    println!(\"Hello\");\n}";
        let result = convert_to_markdown(code, Some("rust"), None);
        assert!(result.contains("```rust"));
        assert!(result.contains("fn main()"));
        assert!(result.contains("```"));
    }

    #[test]
    fn test_convert_with_title() {
        let content = "Some content";
        let result = convert_to_markdown(content, None, Some("My File"));
        assert!(result.contains("# My File"));
    }
}
