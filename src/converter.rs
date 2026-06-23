/// Convert text content to Markdown format (kept for backward compatibility)
#[allow(dead_code)]
pub fn convert_to_markdown(content: &str, language: Option<&str>, title: Option<&str>) -> String {
    convert_to_markdown_with_options(content, language, title, false)
}

/// Convert text content to Markdown with optional line numbers
pub fn convert_to_markdown_with_options(
    content: &str,
    language: Option<&str>,
    title: Option<&str>,
    add_line_numbers: bool,
) -> String {
    let mut markdown = String::new();

    // Add title if provided
    if let Some(title) = title {
        markdown.push_str(&format!("# {}\n\n", title));
    }

    let lang = language.unwrap_or("").trim();

    // Start code block
    markdown.push_str(&format!("```{}\n", lang));

    // Add content with optional line numbers
    if add_line_numbers {
        for (i, line) in content.lines().enumerate() {
            let line_num = i + 1;
            markdown.push_str(&format!("{:4} | {}\n", line_num, line));
        }
    } else {
        markdown.push_str(content);
        if !content.ends_with('\n') {
            markdown.push('\n');
        }
    }

    markdown.push_str("```\n");

    markdown
}

/// Get a language hint/info comment suitable for the language
#[allow(dead_code)]
pub fn get_language_hint(language: &str) -> String {
    match language.to_lowercase().as_str() {
        "python" => "Python code".to_string(),
        "rust" => "Rust code".to_string(),
        "javascript" => "JavaScript code".to_string(),
        "typescript" => "TypeScript code".to_string(),
        "java" => "Java code".to_string(),
        "cpp" | "c++" => "C++ code".to_string(),
        "c" => "C code".to_string(),
        "csharp" => "C# code".to_string(),
        "go" => "Go code".to_string(),
        "ruby" => "Ruby code".to_string(),
        "php" => "PHP code".to_string(),
        "html" => "HTML markup".to_string(),
        "css" => "CSS stylesheet".to_string(),
        "json" => "JSON data".to_string(),
        "yaml" => "YAML configuration".to_string(),
        "sql" => "SQL query".to_string(),
        "bash" => "Bash shell script".to_string(),
        "dockerfile" => "Dockerfile".to_string(),
        "makefile" => "Makefile".to_string(),
        _ => format!("{} code", language),
    }
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
