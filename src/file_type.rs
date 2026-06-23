use std::path::Path;

/// Detect the programming language from file extension
pub fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => match ext.to_lowercase().as_str() {
            // Web
            "html" | "htm" => "html",
            "css" => "css",
            "scss" => "scss",
            "sass" => "sass",
            "less" => "less",
            "js" => "javascript",
            "jsx" => "javascript",
            "mjs" => "javascript",
            "cjs" => "javascript",
            "ts" => "typescript",
            "tsx" => "typescript",
            "mts" => "typescript",
            "vue" => "vue",
            "svelte" => "svelte",
            "astro" => "astro",

            // Server-side
            "py" => "python",
            "pyx" => "python",
            "rb" => "ruby",
            "erb" => "ruby",
            "php" => "php",
            "phtml" => "php",
            "java" => "java",
            "cs" | "csharp" => "csharp",
            "cpp" | "cc" | "cxx" | "c++" => "cpp",
            "c" => "c",
            "h" => "c",
            "hpp" | "hxx" => "cpp",
            "rs" => "rust",
            "go" => "go",
            "kt" | "kts" => "kotlin",
            "swift" => "swift",
            "m" => "objc",
            "mm" => "objc",
            "scala" => "scala",
            "groovy" => "groovy",
            "gradle" => "gradle",
            "vb" | "vbs" => "vbnet",
            "asp" | "aspx" => "asp",
            "jsp" => "jsp",

            // Shell & scripting
            "sh" | "bash" | "zsh" | "fish" => "bash",
            "ps1" | "psm1" => "powershell",
            "cmd" | "bat" => "batch",
            "pl" | "pm" => "perl",
            "awk" => "awk",
            "sed" => "sed",

            // Data & config
            "json" | "jsonc" => "json",
            "yaml" | "yml" => "yaml",
            "xml" => "xml",
            "toml" => "toml",
            "ini" | "cfg" | "conf" => "ini",
            "properties" => "properties",
            "sql" | "psql" => "sql",
            "graphql" | "gql" => "graphql",
            "proto" => "protobuf",

            // Markup & docs
            "md" | "markdown" | "mdown" => "markdown",
            "rst" => "rst",
            "tex" | "latex" => "latex",
            "adoc" => "asciidoc",

            // Build & config
            "dockerfile" => "dockerfile",
            "makefile" | "mk" => "makefile",
            "cmake" => "cmake",
            "ninja" => "ninja",
            "scons" => "python",

            // Data science & analytics
            "r" | "rmd" => "r",
            "lua" => "lua",
            "julia" => "julia",

            // Functional & others
            "clj" | "cljs" => "clojure",
            "ex" | "exs" => "elixir",
            "erl" | "hrl" => "erlang",
            "hx" => "haxe",
            "hs" => "haskell",
            "ml" | "mli" => "ocaml",
            "fs" | "fsx" | "fsi" => "fsharp",
            "fth" => "forth",
            "vim" => "vim",
            "diff" | "patch" => "diff",
            "dart" => "dart",
            "nim" => "nim",
            "jl" => "julia",
            "zig" => "zig",
            "d" => "d",

            // Lisp family
            "lisp" | "lsp" => "lisp",
            "scm" => "scheme",
            "rkt" => "racket",

            // Query languages
            "cypher" => "cypher",
            "sparql" => "sparql",
            "aql" => "aql",

            // Assembly & low-level
            "asm" | "s" => "asm",
            "hex" => "hex",

            _ => "", // Unknown extension - return empty string
        }.to_string(),
        None => String::new(),
    }
}

/// Detect language from filename when extension is missing
pub fn detect_language_from_filename(filename: &str) -> String {
    let lower = filename.to_lowercase();
    match lower.as_str() {
        "dockerfile" => "dockerfile".to_string(),
        "makefile" | "gnumakefile" => "makefile".to_string(),
        "rakefile" => "ruby".to_string(),
        "gemfile" => "ruby".to_string(),
        "podfile" => "ruby".to_string(),
        "fastfile" => "ruby".to_string(),
        "procfile" => "bash".to_string(),
        "capfile" => "ruby".to_string(),
        "thorfile" => "ruby".to_string(),
        "berksfile" => "ruby".to_string(),
        "cheffile" => "ruby".to_string(),
        ".bashrc" | ".bash_profile" | ".zshrc" => "bash".to_string(),
        ".bash_aliases" => "bash".to_string(),
        ".gitignore" | ".gitattributes" => "properties".to_string(),
        ".dockerignore" => "properties".to_string(),
        ".eslintrc" | ".eslintrc.json" => "json".to_string(),
        ".prettierrc" => "json".to_string(),
        ".editorconfig" => "properties".to_string(),
        "tsconfig.json" | "jsconfig.json" => "json".to_string(),
        "package.json" | "package-lock.json" => "json".to_string(),
        "composer.json" => "json".to_string(),
        "pubspec.yaml" => "yaml".to_string(),
        "cargo.toml" | "cargo.lock" => "toml".to_string(),
        "pipfile" => "python".to_string(),
        "requirements.txt" => "properties".to_string(),
        "setup.py" => "python".to_string(),
        "setup.cfg" => "ini".to_string(),
        "pyproject.toml" => "toml".to_string(),
        ".htaccess" => "apache".to_string(),
        "nginx.conf" => "nginx".to_string(),
        "apache.conf" => "apache".to_string(),
        _ => String::new(),
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
