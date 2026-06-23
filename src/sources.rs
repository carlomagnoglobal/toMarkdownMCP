use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use url::Url;

/// Different source types for code
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SourceType {
    FilePath(String),
    Url(String),
    Directory(String),
    Stdin,
}

impl SourceType {
    /// Parse a source identifier into a SourceType
    pub fn from_string(source: &str) -> Result<Self> {
        let trimmed = source.trim();

        // Check for URL
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            Url::parse(trimmed).map_err(|e| anyhow!("Invalid URL: {}", e))?;
            return Ok(SourceType::Url(trimmed.to_string()));
        }

        // Check for stdin
        if trimmed == "-" || trimmed == "stdin" {
            return Ok(SourceType::Stdin);
        }

        // Check if it's a directory
        let path = Path::new(trimmed);
        if path.exists() && path.is_dir() {
            return Ok(SourceType::Directory(trimmed.to_string()));
        }

        // Default to file path
        Ok(SourceType::FilePath(trimmed.to_string()))
    }
}

/// Fetch content from various sources
pub async fn fetch_from_source(source: &SourceType) -> Result<String> {
    match source {
        SourceType::FilePath(path) => {
            std::fs::read_to_string(path)
                .map_err(|e| anyhow!("Failed to read file '{}': {}", path, e))
        }
        SourceType::Url(url) => fetch_from_url(url).await,
        SourceType::Directory(_) => {
            Err(anyhow!(
                "Directory source type requires using list_files_in_directory instead"
            ))
        }
        SourceType::Stdin => fetch_from_stdin(),
    }
}

/// Fetch content from a URL
async fn fetch_from_url(url: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch URL: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "HTTP error {}: {}",
            response.status(),
            response.status().canonical_reason().unwrap_or("Unknown")
        ));
    }

    response
        .text()
        .await
        .map_err(|e| anyhow!("Failed to read response: {}", e))
}

/// Fetch content from stdin
fn fetch_from_stdin() -> Result<String> {
    use std::io::Read;

    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|e| anyhow!("Failed to read from stdin: {}", e))?;

    Ok(buffer)
}

/// List code files in a directory
pub fn list_files_in_directory(
    dir_path: &str,
    recursive: bool,
) -> Result<Vec<PathBuf>> {
    let path = Path::new(dir_path);

    if !path.exists() {
        return Err(anyhow!("Directory not found: {}", dir_path));
    }

    if !path.is_dir() {
        return Err(anyhow!("Path is not a directory: {}", dir_path));
    }

    let mut files = Vec::new();
    collect_code_files(path, &mut files, recursive)?;

    files.sort();
    Ok(files)
}

/// Recursively collect code files from a directory
fn collect_code_files(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    recursive: bool,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden files and common non-code directories
        if let Some(name) = path.file_name() {
            if let Some(name_str) = name.to_str() {
                if name_str.starts_with('.') || is_excluded_dir(name_str) {
                    continue;
                }
            }
        }

        if path.is_dir() && recursive {
            collect_code_files(&path, files, recursive)?;
        } else if path.is_file() && is_code_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

/// Check if a file is likely a code file
fn is_code_file(path: &Path) -> bool {
    let code_extensions = [
        // Programming languages
        "rs", "py", "js", "ts", "go", "java", "cs", "cpp", "c", "h", "hpp",
        "rb", "php", "swift", "kt", "scala", "groovy", "lua", "perl", "pl",
        "sh", "bash", "zsh", "fish", "ps1", "cmd", "bat",
        // Web
        "html", "css", "scss", "sass", "less", "jsx", "tsx", "vue", "svelte",
        // Data & config
        "json", "yaml", "yml", "xml", "toml", "ini", "conf", "cfg", "sql",
        // Build & docs
        "makefile", "dockerfile", "gradle", "cmake", "md", "rst", "tex",
        // Markup
        "xml", "svg",
    ];

    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            return code_extensions.contains(&ext_str.to_lowercase().as_str());
        }
    }

    // Check for common code files without extensions
    if let Some(name) = path.file_name() {
        if let Some(name_str) = name.to_str() {
            return matches!(
                name_str,
                "Dockerfile"
                    | "Makefile"
                    | "Rakefile"
                    | "Gemfile"
                    | "Procfile"
                    | ".gitignore"
                    | ".dockerignore"
                    | ".bashrc"
                    | ".zshrc"
                    | "package.json"
                    | "tsconfig.json"
            );
        }
    }

    false
}

/// Check if a directory should be excluded from scanning
fn is_excluded_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | "target"
            | ".git"
            | ".svn"
            | ".hg"
            | "dist"
            | "build"
            | "out"
            | "__pycache__"
            | ".venv"
            | "venv"
            | ".env"
            | "vendor"
            | ".idea"
            | ".vscode"
            | "coverage"
            | ".pytest_cache"
            | ".cargo"
            | "Pods"
            | ".bundle"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_path() {
        let source = SourceType::from_string("src/main.rs").unwrap();
        match source {
            SourceType::FilePath(path) => assert_eq!(path, "src/main.rs"),
            _ => panic!("Expected FilePath"),
        }
    }

    #[test]
    fn test_parse_url() {
        let source = SourceType::from_string("https://example.com/code.py").unwrap();
        match source {
            SourceType::Url(url) => assert!(url.contains("example.com")),
            _ => panic!("Expected Url"),
        }
    }

    #[test]
    fn test_parse_stdin() {
        let source = SourceType::from_string("-").unwrap();
        match source {
            SourceType::Stdin => {}
            _ => panic!("Expected Stdin"),
        }
    }

    #[test]
    fn test_is_code_file() {
        assert!(is_code_file(Path::new("script.py")));
        assert!(is_code_file(Path::new("main.rs")));
        assert!(is_code_file(Path::new("index.html")));
        assert!(!is_code_file(Path::new("image.png")));
        assert!(!is_code_file(Path::new("doc.pdf")));
    }

    #[test]
    fn test_is_excluded_dir() {
        assert!(is_excluded_dir("node_modules"));
        assert!(is_excluded_dir("target"));
        assert!(is_excluded_dir(".git"));
        assert!(!is_excluded_dir("src"));
        assert!(!is_excluded_dir("lib"));
    }
}
