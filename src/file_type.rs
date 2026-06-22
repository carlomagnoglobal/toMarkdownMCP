use std::path::Path;

/// Detect the programming language from file extension
pub fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => match ext.to_lowercase().as_str() {
            // Web
            "html" => "html",
            "htm" => "html",
            "css" => "css",
            "scss" => "scss",
            "sass" => "sass",
            "less" => "less",
            "js" => "javascript",
            "jsx" => "javascript",
            "ts" => "typescript",
            "tsx" => "typescript",
            "vue" => "vue",

            // Server-side
            "py" => "python",
            "rb" => "ruby",
            "php" => "php",
            "java" => "java",
            "cs" => "csharp",
            "cpp" | "cc" | "cxx" | "c++" => "cpp",
            "c" => "c",
            "h" | "hpp" => "cpp",
            "rs" => "rust",
            "go" => "go",
            "kt" => "kotlin",
            "swift" => "swift",
            "m" => "objc",
            "scala" => "scala",
            "groovy" => "groovy",

            // Shell & scripting
            "sh" => "bash",
            "bash" => "bash",
            "zsh" => "bash",
            "fish" => "fish",
            "ps1" => "powershell",
            "cmd" => "batch",
            "bat" => "batch",

            // Data & config
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "xml" => "xml",
            "toml" => "toml",
            "ini" => "ini",
            "cfg" | "conf" => "properties",
            "sql" => "sql",
            "graphql" => "graphql",

            // Markup & docs
            "md" | "markdown" => "markdown",
            "rst" => "rst",
            "tex" => "latex",

            // Other
            "dockerfile" => "dockerfile",
            "makefile" => "makefile",
            "gradle" => "gradle",
            "r" => "r",
            "lua" => "lua",
            "perl" => "perl",
            "pl" => "perl",
            "clj" => "clojure",
            "cljs" => "clojure",
            "ex" => "elixir",
            "exs" => "elixir",
            "erl" => "erlang",
            "hrl" => "erlang",
            "hx" => "haxe",

            _ => "", // Unknown extension - return empty string
        }.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust() {
        let path = Path::new("main.rs");
        assert_eq!(detect_language(path), "rust");
    }

    #[test]
    fn test_detect_python() {
        let path = Path::new("script.py");
        assert_eq!(detect_language(path), "python");
    }

    #[test]
    fn test_detect_javascript() {
        let path = Path::new("app.js");
        assert_eq!(detect_language(path), "javascript");
    }

    #[test]
    fn test_detect_json() {
        let path = Path::new("config.json");
        assert_eq!(detect_language(path), "json");
    }

    #[test]
    fn test_unknown_extension() {
        let path = Path::new("file.unknown");
        assert_eq!(detect_language(path), "");
    }

    #[test]
    fn test_case_insensitive() {
        let path = Path::new("file.PY");
        assert_eq!(detect_language(path), "python");
    }
}
