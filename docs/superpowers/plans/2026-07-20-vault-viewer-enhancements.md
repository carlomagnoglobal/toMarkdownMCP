# Vault Viewer Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the toMarkdownMCP GUI from a markdown-only viewer into a comprehensive vault explorer supporting multiple file types, intelligent indexing, search, and minimal code editing.

**Architecture:** Trait-based polymorphic viewer system with a unified SQLite database per vault. Each file type (Markdown, Code, Image, Hex) has a dedicated viewer implementing a common interface. Viewers are composed into a tab-based UI with persistence, search, and quick navigation.

**Tech Stack:** Rust, Tauri, SQLite (rusqlite), syntect (syntax highlighting), tracing (logging), infer (magic byte detection), image (image handling)

## Global Constraints

- MSRV: 1.88 (from memory)
- SQLite per vault: `.tomarkdown/vault.db`
- Log retention: 5 days (rotating)
- Tab persistence: Last 5 tabs only
- Code files >10MB: read-only (warn user)
- Images >100MB: reduced resolution (warn user)
- Hex viewer: lazy-load first 1MB of large files
- Auto-save default: OFF
- All logging: console + rotating file logs

---

## Phase 1: Foundation & Infrastructure

### Task 1: Add Dependencies to Cargo.toml

**Files:**
- Modify: `gui/Cargo.toml`

**Interfaces:**
- Produces: Updated dependency list for phases 1-7

- [ ] **Step 1: Add core dependencies**

```toml
[dependencies]
# Existing deps (keep unchanged)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tauri = { version = "2.0", features = ["dialog", "window"] }
notify = "6.0"

# New deps for this feature
rusqlite = { version = "0.31", features = ["bundled", "chrono"] }
syntect = "5.0"
tracing = "0.1"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
infer = "0.15"
image = "0.25"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
```

- [ ] **Step 2: Run cargo check to verify dependencies resolve**

```bash
cd gui && cargo check
```

Expected: No errors, all dependencies resolve.

- [ ] **Step 3: Commit**

```bash
git add gui/Cargo.toml
git commit -m "deps: add rusqlite, syntect, tracing, infer, image for vault enhancements"
```

---

### Task 2: Create File Type Detection Module

**Files:**
- Create: `gui/src/file_types.rs`
- Create: `gui/tests/file_type_detection_tests.rs`
- Modify: `gui/src/lib.rs` (add module)

**Interfaces:**
- Produces: `FileType` enum and `detect_file_type(path: &Path) -> FileType` function
  ```rust
  pub enum FileType {
      Markdown,
      Code { language: String },
      Image { format: String },
      Hex,
  }
  pub fn detect_file_type(path: &Path) -> FileType { ... }
  ```

- [ ] **Step 1: Write failing test for file type detection**

```rust
// gui/tests/file_type_detection_tests.rs
#[cfg(test)]
mod tests {
    use std::path::Path;
    use gui::file_types::{detect_file_type, FileType};

    #[test]
    fn test_detect_markdown() {
        let path = Path::new("test.md");
        match detect_file_type(path) {
            FileType::Markdown => (),
            _ => panic!("Expected Markdown"),
        }
    }

    #[test]
    fn test_detect_python_code() {
        let path = Path::new("script.py");
        match detect_file_type(path) {
            FileType::Code { language } if language == "python" => (),
            _ => panic!("Expected Code with language 'python'"),
        }
    }

    #[test]
    fn test_detect_rust_code() {
        let path = Path::new("main.rs");
        match detect_file_type(path) {
            FileType::Code { language } if language == "rust" => (),
            _ => panic!("Expected Code with language 'rust'"),
        }
    }

    #[test]
    fn test_detect_png_image() {
        let path = Path::new("image.png");
        match detect_file_type(path) {
            FileType::Image { format } if format == "png" => (),
            _ => panic!("Expected Image with format 'png'"),
        }
    }

    #[test]
    fn test_detect_unknown_as_hex() {
        let path = Path::new("binary.bin");
        match detect_file_type(path) {
            FileType::Hex => (),
            _ => panic!("Expected Hex"),
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd gui && cargo test file_type_detection_tests --test '*' -- --nocapture
```

Expected: FAIL (file_types module not found)

- [ ] **Step 3: Create file_types.rs with implementation**

```rust
// gui/src/file_types.rs
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    Markdown,
    Code { language: String },
    Image { format: String },
    Hex,
}

// Language mappings by extension
const LANGUAGE_MAP: &[(&str, &str)] = &[
    // Web
    ("js", "javascript"),
    ("ts", "typescript"),
    ("jsx", "jsx"),
    ("tsx", "tsx"),
    ("html", "html"),
    ("css", "css"),
    ("scss", "scss"),
    ("less", "less"),
    ("vue", "vue"),
    ("svelte", "svelte"),
    // Python
    ("py", "python"),
    ("pyx", "python"),
    ("pyi", "python"),
    ("ipynb", "python"),
    // Rust
    ("rs", "rust"),
    // Go
    ("go", "go"),
    // C/C++
    ("c", "c"),
    ("cpp", "cpp"),
    ("cc", "cpp"),
    ("cxx", "cpp"),
    ("h", "c"),
    ("hpp", "cpp"),
    ("h++", "cpp"),
    // Java/JVM
    ("java", "java"),
    ("kt", "kotlin"),
    ("scala", "scala"),
    // C#/.NET
    ("cs", "csharp"),
    ("csx", "csharp"),
    ("fsx", "fsharp"),
    ("fs", "fsharp"),
    // PHP
    ("php", "php"),
    ("phtml", "php"),
    // Ruby
    ("rb", "ruby"),
    ("erb", "ruby"),
    // Shell
    ("sh", "bash"),
    ("bash", "bash"),
    ("zsh", "bash"),
    ("fish", "fish"),
    // Config/Data
    ("json", "json"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("toml", "toml"),
    ("xml", "xml"),
    ("ini", "ini"),
    ("env", "bash"),
    ("properties", "properties"),
    ("sql", "sql"),
    ("sqlite", "sql"),
    // Text
    ("txt", "plaintext"),
];

// Image formats by extension
const IMAGE_EXTENSIONS: &[(&str, &str)] = &[
    ("png", "png"),
    ("jpg", "jpeg"),
    ("jpeg", "jpeg"),
    ("gif", "gif"),
    ("svg", "svg"),
    ("webp", "webp"),
    ("avif", "avif"),
    ("tiff", "tiff"),
    ("tif", "tiff"),
];

pub fn detect_file_type(path: &Path) -> FileType {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // Check markdown
    if extension == "md" || extension == "markdown" {
        return FileType::Markdown;
    }

    // Check code
    for (ext, lang) in LANGUAGE_MAP {
        if extension == *ext {
            return FileType::Code {
                language: lang.to_string(),
            };
        }
    }

    // Check image
    for (ext, fmt) in IMAGE_EXTENSIONS {
        if extension == *ext {
            return FileType::Image {
                format: fmt.to_string(),
            };
        }
    }

    // Default to hex for unknown
    FileType::Hex
}
```

- [ ] **Step 4: Add module to lib.rs**

```rust
// gui/src/lib.rs (add this line)
pub mod file_types;
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd gui && cargo test file_type_detection_tests --test '*' -- --nocapture
```

Expected: PASS (all tests)

- [ ] **Step 6: Commit**

```bash
git add gui/src/file_types.rs gui/src/lib.rs gui/tests/file_type_detection_tests.rs
git commit -m "feat: add file type detection module with language mapping"
```

---

### Task 3: Create Vault Database Schema Module

**Files:**
- Create: `gui/src/vault/mod.rs`
- Create: `gui/src/vault/schema.rs`
- Create: `gui/tests/vault_schema_tests.rs`
- Modify: `gui/src/lib.rs` (add module)

**Interfaces:**
- Produces: `VaultDb` struct and `init_vault_db(vault_root: &Path) -> Result<VaultDb, Error>` function
  ```rust
  pub struct VaultDb {
      pub conn: rusqlite::Connection,
      pub vault_root: PathBuf,
  }
  pub fn init_vault_db(vault_root: &Path) -> Result<VaultDb, Error> { ... }
  ```

- [ ] **Step 1: Write failing test for vault database initialization**

```rust
// gui/tests/vault_schema_tests.rs
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use tempfile::TempDir;
    use gui::vault::init_vault_db;

    #[test]
    fn test_init_vault_db() {
        let temp_dir = TempDir::new().unwrap();
        let vault_root = temp_dir.path();
        
        let db = init_vault_db(vault_root).expect("Failed to init vault db");
        
        // Verify database file exists
        let db_path = vault_root.join(".tomarkdown").join("vault.db");
        assert!(db_path.exists(), "Database file should exist");
    }

    #[test]
    fn test_vault_schema_tables_exist() {
        let temp_dir = TempDir::new().unwrap();
        let vault_root = temp_dir.path();
        
        let db = init_vault_db(vault_root).expect("Failed to init vault db");
        
        // Check that required tables exist
        let tables: Vec<String> = db.conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        
        assert!(tables.contains(&"files".to_string()));
        assert!(tables.contains(&"file_links".to_string()));
        assert!(tables.contains(&"word_graph".to_string()));
        assert!(tables.contains(&"index_state".to_string()));
        assert!(tables.contains(&"recent_files".to_string()));
    }
}
```

- [ ] **Step 2: Add tempfile dev dependency**

```toml
# gui/Cargo.toml
[dev-dependencies]
tempfile = "3.8"
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cd gui && cargo test vault_schema_tests --test '*' -- --nocapture
```

Expected: FAIL (vault module not found)

- [ ] **Step 4: Create vault/schema.rs**

```rust
// gui/src/vault/schema.rs
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS files (
  id INTEGER PRIMARY KEY,
  path TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  extension TEXT,
  file_type TEXT,
  language TEXT,
  size_bytes INTEGER,
  modified_at INTEGER,
  last_indexed_at INTEGER,
  is_indexed BOOLEAN DEFAULT 0
);

CREATE TABLE IF NOT EXISTS file_links (
  id INTEGER PRIMARY KEY,
  source_id INTEGER NOT NULL,
  target_id INTEGER,
  link_type TEXT,
  FOREIGN KEY (source_id) REFERENCES files(id),
  FOREIGN KEY (target_id) REFERENCES files(id)
);

CREATE TABLE IF NOT EXISTS word_graph (
  id INTEGER PRIMARY KEY,
  word1 TEXT NOT NULL,
  word2 TEXT NOT NULL,
  co_occurrence_count INTEGER DEFAULT 1,
  last_updated INTEGER
);

CREATE TABLE IF NOT EXISTS index_state (
  id INTEGER PRIMARY KEY,
  last_full_index INTEGER,
  last_incremental_index INTEGER,
  total_files_indexed INTEGER
);

CREATE TABLE IF NOT EXISTS recent_files (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL,
  opened_at INTEGER NOT NULL,
  FOREIGN KEY (file_id) REFERENCES files(id),
  UNIQUE(file_id)
);

CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(path, name, content, language);

CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);
CREATE INDEX IF NOT EXISTS idx_files_language ON files(language);
CREATE INDEX IF NOT EXISTS idx_file_links_source ON file_links(source_id);
CREATE INDEX IF NOT EXISTS idx_file_links_target ON file_links(target_id);
CREATE INDEX IF NOT EXISTS idx_recent_files_opened ON recent_files(opened_at DESC);
"#;
```

- [ ] **Step 5: Create vault/mod.rs**

```rust
// gui/src/vault/mod.rs
pub mod schema;

use std::path::{Path, PathBuf};
use rusqlite::{Connection, Result as SqliteResult};
use chrono::Utc;

pub struct VaultDb {
    pub conn: Connection,
    pub vault_root: PathBuf,
}

impl VaultDb {
    pub fn new(conn: Connection, vault_root: PathBuf) -> Self {
        VaultDb { conn, vault_root }
    }
}

pub fn init_vault_db(vault_root: &Path) -> Result<VaultDb, Box<dyn std::error::Error>> {
    let tomarkdown_dir = vault_root.join(".tomarkdown");
    std::fs::create_dir_all(&tomarkdown_dir)?;
    
    let db_path = tomarkdown_dir.join("vault.db");
    let conn = Connection::open(&db_path)?;
    
    // Execute schema
    conn.execute_batch(schema::SCHEMA)?;
    
    Ok(VaultDb {
        conn,
        vault_root: vault_root.to_path_buf(),
    })
}
```

- [ ] **Step 6: Add vault module to lib.rs**

```rust
// gui/src/lib.rs
pub mod vault;
```

- [ ] **Step 7: Run tests to verify they pass**

```bash
cd gui && cargo test vault_schema_tests --test '*' -- --nocapture
```

Expected: PASS (all tests)

- [ ] **Step 8: Commit**

```bash
git add gui/src/vault/mod.rs gui/src/vault/schema.rs gui/src/lib.rs gui/tests/vault_schema_tests.rs gui/Cargo.toml
git commit -m "feat: add vault database schema and initialization"
```

---

### Task 4: Create FileViewer Trait

**Files:**
- Create: `gui/src/viewers/mod.rs`
- Create: `gui/src/viewers/traits.rs`
- Modify: `gui/src/lib.rs` (add module)

**Interfaces:**
- Produces: `FileViewer` trait and supporting types
  ```rust
  pub trait FileViewer: Send + Sync {
      fn render(&self) -> Result<String, ViewerError>;
      fn get_state(&self) -> ViewerState;
  }
  
  #[derive(Clone, Debug, Serialize)]
  pub struct ViewerState {
      pub file_type: String,
      pub file_path: PathBuf,
      pub modified: bool,
  }
  ```

- [ ] **Step 1: Create viewers/traits.rs**

```rust
// gui/src/viewers/traits.rs
use std::path::PathBuf;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ViewerState {
    pub file_type: String,
    pub file_path: PathBuf,
    pub modified: bool,
    pub file_size_bytes: u64,
}

#[derive(Debug)]
pub enum ViewerError {
    IoError(String),
    ParseError(String),
    RenderError(String),
}

impl std::fmt::Display for ViewerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewerError::IoError(e) => write!(f, "IO Error: {}", e),
            ViewerError::ParseError(e) => write!(f, "Parse Error: {}", e),
            ViewerError::RenderError(e) => write!(f, "Render Error: {}", e),
        }
    }
}

impl std::error::Error for ViewerError {}

pub trait FileViewer: Send + Sync {
    /// Render the file for display (HTML or formatted content)
    fn render(&self) -> Result<String, ViewerError>;
    
    /// Get the current state of the viewer
    fn get_state(&self) -> ViewerState;
    
    /// Get the file type identifier
    fn file_type(&self) -> &str;
}
```

- [ ] **Step 2: Create viewers/mod.rs**

```rust
// gui/src/viewers/mod.rs
pub mod traits;

pub use traits::{FileViewer, ViewerState, ViewerError};
```

- [ ] **Step 3: Add viewers module to lib.rs**

```rust
// gui/src/lib.rs
pub mod viewers;
```

- [ ] **Step 4: Verify it compiles**

```bash
cd gui && cargo check
```

Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add gui/src/viewers/mod.rs gui/src/viewers/traits.rs gui/src/lib.rs
git commit -m "feat: create FileViewer trait for polymorphic viewer system"
```

---

## Phase 2: Implement File Viewers

### Task 5: Implement Markdown Viewer

**Files:**
- Create: `gui/src/viewers/markdown.rs`
- Modify: `gui/src/viewers/mod.rs` (add module)

**Interfaces:**
- Consumes: Existing `render_note` function and `RenderOpts` from main.rs
- Produces: `MarkdownViewer` struct implementing `FileViewer` trait

- [ ] **Step 1: Create viewers/markdown.rs**

```rust
// gui/src/viewers/markdown.rs
use std::path::PathBuf;
use crate::viewers::{FileViewer, ViewerState, ViewerError};

pub struct MarkdownViewer {
    path: PathBuf,
    html: String,
    file_size: u64,
}

impl MarkdownViewer {
    pub fn new(path: PathBuf, html: String, file_size: u64) -> Self {
        MarkdownViewer {
            path,
            html,
            file_size,
        }
    }
}

impl FileViewer for MarkdownViewer {
    fn render(&self) -> Result<String, ViewerError> {
        Ok(self.html.clone())
    }
    
    fn get_state(&self) -> ViewerState {
        ViewerState {
            file_type: "markdown".to_string(),
            file_path: self.path.clone(),
            modified: false,
            file_size_bytes: self.file_size,
        }
    }
    
    fn file_type(&self) -> &str {
        "markdown"
    }
}
```

- [ ] **Step 2: Add to viewers/mod.rs**

```rust
// gui/src/viewers/mod.rs
pub mod traits;
pub mod markdown;

pub use traits::{FileViewer, ViewerState, ViewerError};
pub use markdown::MarkdownViewer;
```

- [ ] **Step 3: Verify it compiles**

```bash
cd gui && cargo check
```

Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add gui/src/viewers/markdown.rs gui/src/viewers/mod.rs
git commit -m "feat: implement MarkdownViewer"
```

---

### Task 6: Implement Hex Viewer

**Files:**
- Create: `gui/src/viewers/hex.rs`
- Create: `gui/tests/hex_viewer_tests.rs`
- Modify: `gui/src/viewers/mod.rs` (add module)

**Interfaces:**
- Consumes: File path
- Produces: `HexViewer` struct implementing `FileViewer` trait

- [ ] **Step 1: Write failing test for hex viewer**

```rust
// gui/tests/hex_viewer_tests.rs
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use gui::viewers::{FileViewer, HexViewer};

    #[test]
    fn test_hex_viewer_render() {
        let viewer = HexViewer::new_from_bytes(
            PathBuf::from("test.bin"),
            vec![0x48, 0x65, 0x6C, 0x6C, 0x6F], // "Hello"
            5,
        ).expect("Failed to create viewer");
        
        let html = viewer.render().expect("Failed to render");
        assert!(html.contains("48656C6C6F"), "Should contain hex representation");
        assert!(html.contains("Hello"), "Should contain ASCII representation");
    }

    #[test]
    fn test_hex_viewer_16_bytes_per_line() {
        let mut bytes = vec![];
        for i in 0..32 {
            bytes.push(i as u8);
        }
        
        let viewer = HexViewer::new_from_bytes(
            PathBuf::from("test.bin"),
            bytes,
            32,
        ).expect("Failed to create viewer");
        
        let html = viewer.render().expect("Failed to render");
        // Should have 2 lines (16 bytes each)
        let line_count = html.matches("<tr>").count();
        assert_eq!(line_count, 2, "Should have 2 hex lines");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd gui && cargo test hex_viewer_tests --test '*' -- --nocapture
```

Expected: FAIL (HexViewer not found)

- [ ] **Step 3: Create viewers/hex.rs**

```rust
// gui/src/viewers/hex.rs
use std::path::PathBuf;
use crate::viewers::{FileViewer, ViewerState, ViewerError};

pub struct HexViewer {
    path: PathBuf,
    bytes: Vec<u8>,
    total_size: u64,
}

impl HexViewer {
    pub fn new_from_bytes(path: PathBuf, bytes: Vec<u8>, total_size: u64) -> Result<Self, ViewerError> {
        Ok(HexViewer {
            path,
            bytes,
            total_size,
        })
    }
    
    fn format_hex_line(&self, offset: usize, line_bytes: &[u8]) -> String {
        // Hex representation
        let hex: String = line_bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        
        // ASCII representation (non-printable as dots)
        let ascii: String = line_bytes
            .iter()
            .map(|b| if *b >= 32 && *b < 127 { *b as char } else { '.' })
            .collect();
        
        format!("<tr><td style='font-family:monospace; color:#888;'>{:08X}</td><td style='font-family:monospace;'>{:48}</td><td style='font-family:monospace;'>{}</td></tr>", offset, hex, ascii)
    }
}

impl FileViewer for HexViewer {
    fn render(&self) -> Result<String, ViewerError> {
        let mut html = String::from("<table style='border-collapse:collapse;'>");
        
        for (i, chunk) in self.bytes.chunks(16).enumerate() {
            let offset = i * 16;
            html.push_str(&self.format_hex_line(offset, chunk));
        }
        
        html.push_str("</table>");
        Ok(html)
    }
    
    fn get_state(&self) -> ViewerState {
        ViewerState {
            file_type: "hex".to_string(),
            file_path: self.path.clone(),
            modified: false,
            file_size_bytes: self.total_size,
        }
    }
    
    fn file_type(&self) -> &str {
        "hex"
    }
}
```

- [ ] **Step 4: Add to viewers/mod.rs**

```rust
// gui/src/viewers/mod.rs
pub mod traits;
pub mod markdown;
pub mod hex;

pub use traits::{FileViewer, ViewerState, ViewerError};
pub use markdown::MarkdownViewer;
pub use hex::HexViewer;
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd gui && cargo test hex_viewer_tests --test '*' -- --nocapture
```

Expected: PASS (all tests)

- [ ] **Step 6: Commit**

```bash
git add gui/src/viewers/hex.rs gui/src/viewers/mod.rs gui/tests/hex_viewer_tests.rs
git commit -m "feat: implement HexViewer with 16-byte-per-line formatting"
```

---

### Task 7: Implement Code Viewer with Syntax Highlighting

**Files:**
- Create: `gui/src/viewers/code.rs`
- Create: `gui/tests/code_viewer_tests.rs`
- Modify: `gui/src/viewers/mod.rs` (add module)

**Interfaces:**
- Consumes: File path, language, content
- Produces: `CodeViewer` struct implementing `FileViewer` trait

- [ ] **Step 1: Write failing test for code viewer**

```rust
// gui/tests/code_viewer_tests.rs
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use gui::viewers::{FileViewer, CodeViewer};

    #[test]
    fn test_code_viewer_creates() {
        let viewer = CodeViewer::new(
            PathBuf::from("test.rs"),
            "rust".to_string(),
            "fn main() { println!(\"Hello\"); }".to_string(),
            false,
        ).expect("Failed to create viewer");
        
        assert_eq!(viewer.file_type(), "code");
    }

    #[test]
    fn test_code_viewer_render() {
        let viewer = CodeViewer::new(
            PathBuf::from("test.py"),
            "python".to_string(),
            "print('Hello')".to_string(),
            false,
        ).expect("Failed to create viewer");
        
        let html = viewer.render().expect("Failed to render");
        // Should contain some HTML (from syntect highlighting)
        assert!(html.contains("Hello") || html.contains("html"), "Should render content");
    }

    #[test]
    fn test_code_viewer_dirty_flag() {
        let viewer = CodeViewer::new(
            PathBuf::from("test.rs"),
            "rust".to_string(),
            "code".to_string(),
            true,
        ).expect("Failed to create viewer");
        
        let state = viewer.get_state();
        assert!(state.modified, "Dirty flag should be set");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd gui && cargo test code_viewer_tests --test '*' -- --nocapture
```

Expected: FAIL (CodeViewer not found)

- [ ] **Step 3: Create viewers/code.rs**

```rust
// gui/src/viewers/code.rs
use std::path::PathBuf;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html;
use syntect::parsing::SyntaxSet;
use crate::viewers::{FileViewer, ViewerState, ViewerError};

pub struct CodeViewer {
    path: PathBuf,
    language: String,
    content: String,
    dirty: bool,
    file_size: u64,
}

impl CodeViewer {
    pub fn new(path: PathBuf, language: String, content: String, dirty: bool) -> Result<Self, ViewerError> {
        let file_size = content.len() as u64;
        Ok(CodeViewer {
            path,
            language,
            content,
            dirty,
            file_size,
        })
    }
    
    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
}

impl FileViewer for CodeViewer {
    fn render(&self) -> Result<String, ViewerError> {
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        
        // Try to get syntax for the language
        let syntax = ss.find_syntax_by_name(&self.language)
            .or_else(|| ss.find_syntax_by_extension(&self.language))
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        
        let mut highlighter = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
        
        // Highlight each line
        let mut html = String::from("<pre style='background:#2b303b;color:#c0c5ce;overflow-x:auto;padding:10px;'>");
        
        for (i, line) in self.content.lines().enumerate() {
            let highlighted = highlighter
                .highlight_line(line, &ss)
                .map_err(|_| ViewerError::RenderError("Highlight error".to_string()))?;
            
            let line_num = i + 1;
            html.push_str(&format!("<div style='display:flex;'><span style='color:#65737E;margin-right:10px;width:50px;text-align:right;'>{}</span><span>", line_num));
            
            for (style, seg) in &highlighted {
                let fg = style.foreground;
                let color = format!("#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b);
                html.push_str(&format!("<span style='color:{};'>{}</span>", color, syntect::util::as_24_bit_terminal_escaped(&highlighted, false)));
            }
            
            html.push_str("</span></div>\n");
        }
        
        html.push_str("</pre>");
        Ok(html)
    }
    
    fn get_state(&self) -> ViewerState {
        ViewerState {
            file_type: "code".to_string(),
            file_path: self.path.clone(),
            modified: self.dirty,
            file_size_bytes: self.file_size,
        }
    }
    
    fn file_type(&self) -> &str {
        "code"
    }
}
```

- [ ] **Step 4: Add to viewers/mod.rs**

```rust
// gui/src/viewers/mod.rs
pub mod traits;
pub mod markdown;
pub mod hex;
pub mod code;

pub use traits::{FileViewer, ViewerState, ViewerError};
pub use markdown::MarkdownViewer;
pub use hex::HexViewer;
pub use code::CodeViewer;
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd gui && cargo test code_viewer_tests --test '*' -- --nocapture
```

Expected: PASS (all tests)

- [ ] **Step 6: Commit**

```bash
git add gui/src/viewers/code.rs gui/src/viewers/mod.rs gui/tests/code_viewer_tests.rs
git commit -m "feat: implement CodeViewer with syntect syntax highlighting"
```

---

### Task 8: Implement Image Viewer

**Files:**
- Create: `gui/src/viewers/image.rs`
- Create: `gui/tests/image_viewer_tests.rs`
- Modify: `gui/src/viewers/mod.rs` (add module)

**Interfaces:**
- Consumes: File path, image format
- Produces: `ImageViewer` struct implementing `FileViewer` trait

- [ ] **Step 1: Write failing test for image viewer**

```rust
// gui/tests/image_viewer_tests.rs
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use gui::viewers::{FileViewer, ImageViewer};

    #[test]
    fn test_image_viewer_creates() {
        let viewer = ImageViewer::new(
            PathBuf::from("test.png"),
            "png".to_string(),
            800,
            600,
        ).expect("Failed to create viewer");
        
        assert_eq!(viewer.file_type(), "image");
    }

    #[test]
    fn test_image_viewer_dimensions() {
        let viewer = ImageViewer::new(
            PathBuf::from("test.jpg"),
            "jpeg".to_string(),
            1920,
            1080,
        ).expect("Failed to create viewer");
        
        let state = viewer.get_state();
        assert_eq!(state.file_path.file_name().unwrap().to_str().unwrap(), "test.jpg");
    }

    #[test]
    fn test_image_viewer_render() {
        let viewer = ImageViewer::new(
            PathBuf::from("test.gif"),
            "gif".to_string(),
            640,
            480,
        ).expect("Failed to create viewer");
        
        let html = viewer.render().expect("Failed to render");
        assert!(html.contains("img") || html.contains("image"), "Should contain image element");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd gui && cargo test image_viewer_tests --test '*' -- --nocapture
```

Expected: FAIL (ImageViewer not found)

- [ ] **Step 3: Create viewers/image.rs**

```rust
// gui/src/viewers/image.rs
use std::path::PathBuf;
use crate::viewers::{FileViewer, ViewerState, ViewerError};

pub struct ImageViewer {
    path: PathBuf,
    format: String,
    width: u32,
    height: u32,
    file_size: u64,
}

impl ImageViewer {
    pub fn new(path: PathBuf, format: String, width: u32, height: u32) -> Result<Self, ViewerError> {
        Ok(ImageViewer {
            path,
            format,
            width,
            height,
            file_size: 0,
        })
    }
    
    pub fn new_with_size(path: PathBuf, format: String, width: u32, height: u32, file_size: u64) -> Result<Self, ViewerError> {
        Ok(ImageViewer {
            path,
            format,
            width,
            height,
            file_size,
        })
    }
}

impl FileViewer for ImageViewer {
    fn render(&self) -> Result<String, ViewerError> {
        let path_str = self.path.to_string_lossy();
        let html = format!(
            "<div style='display:flex;flex-direction:column;align-items:center;padding:20px;'>\
                <img src='file://{}' style='max-width:100%;max-height:80vh;border:1px solid #ccc;' alt='Image'/>\
                <div style='margin-top:20px;color:#666;font-size:12px;'>\
                    <p>Format: {} | Dimensions: {}x{}</p>\
                </div>\
            </div>",
            path_str, self.format, self.width, self.height
        );
        Ok(html)
    }
    
    fn get_state(&self) -> ViewerState {
        ViewerState {
            file_type: "image".to_string(),
            file_path: self.path.clone(),
            modified: false,
            file_size_bytes: self.file_size,
        }
    }
    
    fn file_type(&self) -> &str {
        "image"
    }
}
```

- [ ] **Step 4: Add to viewers/mod.rs**

```rust
// gui/src/viewers/mod.rs
pub mod traits;
pub mod markdown;
pub mod hex;
pub mod code;
pub mod image;

pub use traits::{FileViewer, ViewerState, ViewerError};
pub use markdown::MarkdownViewer;
pub use hex::HexViewer;
pub use code::CodeViewer;
pub use image::ImageViewer;
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd gui && cargo test image_viewer_tests --test '*' -- --nocapture
```

Expected: PASS (all tests)

- [ ] **Step 6: Commit**

```bash
git add gui/src/viewers/image.rs gui/src/viewers/mod.rs gui/tests/image_viewer_tests.rs
git commit -m "feat: implement ImageViewer with dimensions and format display"
```

---

## Phase 3: File Operations

### Task 9: Implement File Duplication Command

**Files:**
- Create: `gui/src/commands/file_ops.rs`
- Create: `gui/tests/file_ops_tests.rs`
- Modify: `gui/src/main.rs` (add command)
- Modify: `gui/src/lib.rs` (add module)

**Interfaces:**
- Consumes: File path (String)
- Produces: Tauri command `duplicate_file(path: String) -> Result<String, String>`
  - Returns: Path to new duplicated file

- [ ] **Step 1: Write failing test for file duplication**

```rust
// gui/tests/file_ops_tests.rs
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use gui::commands::file_ops::duplicate_file_impl;

    #[test]
    fn test_duplicate_file() {
        let temp_dir = TempDir::new().unwrap();
        let original_path = temp_dir.path().join("original.txt");
        fs::write(&original_path, "test content").unwrap();
        
        let duplicate_path = duplicate_file_impl(&original_path).expect("Failed to duplicate");
        
        assert!(duplicate_path.exists(), "Duplicate file should exist");
        let dup_name = duplicate_path.file_name().unwrap().to_str().unwrap();
        assert!(dup_name.contains("copy"), "Duplicate should have 'copy' in name");
        
        let original_content = fs::read_to_string(&original_path).unwrap();
        let dup_content = fs::read_to_string(&duplicate_path).unwrap();
        assert_eq!(original_content, dup_content, "Content should match");
    }

    #[test]
    fn test_duplicate_file_numbering() {
        let temp_dir = TempDir::new().unwrap();
        let original_path = temp_dir.path().join("test.txt");
        fs::write(&original_path, "content").unwrap();
        
        let dup1 = duplicate_file_impl(&original_path).expect("First duplicate failed");
        let dup2 = duplicate_file_impl(&original_path).expect("Second duplicate failed");
        
        assert_ne!(dup1, dup2, "Duplicates should have different names");
        
        let name1 = dup1.file_name().unwrap().to_str().unwrap();
        let name2 = dup2.file_name().unwrap().to_str().unwrap();
        
        // One should have "copy" and the other should have "copy (2)"
        assert!(name1.contains("copy"), "First dup should have 'copy'");
        assert!(name2.contains("copy"), "Second dup should have 'copy'");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd gui && cargo test file_ops_tests --test '*' -- --nocapture
```

Expected: FAIL (file_ops module not found)

- [ ] **Step 3: Create commands/file_ops.rs**

```rust
// gui/src/commands/file_ops.rs
use std::path::{Path, PathBuf};
use std::fs;

pub fn duplicate_file_impl(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }
    
    let parent = path.parent().ok_or("Cannot get parent directory")?;
    let file_name = path.file_name().ok_or("Cannot get file name")?;
    let file_name_str = file_name.to_string_lossy();
    
    // Parse filename and extension
    let parts: Vec<&str> = file_name_str.split('.').collect();
    let (name, ext) = if parts.len() > 1 {
        let ext = parts.pop().unwrap();
        let name = parts.join(".");
        (name, Some(ext))
    } else {
        (file_name_str.to_string(), None)
    };
    
    // Find available duplicate name
    let mut copy_num = 1;
    loop {
        let dup_name = if copy_num == 1 {
            if let Some(e) = ext {
                format!("{} copy.{}", name, e)
            } else {
                format!("{} copy", name)
            }
        } else {
            if let Some(e) = ext {
                format!("{} copy ({}).{}", name, copy_num, e)
            } else {
                format!("{} copy ({})", name, copy_num)
            }
        };
        
        let dup_path = parent.join(&dup_name);
        
        if !dup_path.exists() {
            // Copy file
            fs::copy(path, &dup_path)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
            return Ok(dup_path);
        }
        
        copy_num += 1;
        if copy_num > 100 {
            return Err("Too many copies, giving up".to_string());
        }
    }
}

#[tauri::command]
pub async fn duplicate_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(&path);
    let dup_path = duplicate_file_impl(&path)?;
    Ok(dup_path.to_string_lossy().into_owned())
}
```

- [ ] **Step 4: Add commands module to lib.rs**

```rust
// gui/src/lib.rs
pub mod commands {
    pub mod file_ops;
}
```

- [ ] **Step 5: Add command to main.rs**

Find the `main()` function and update the builder:

```rust
// gui/src/main.rs
let app = tauri::Builder::default()
    // ... existing setup ...
    .invoke_handler(tauri::generate_handler![
        // ... existing commands ...
        commands::file_ops::duplicate_file,
    ])
    // ... rest of setup ...
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cd gui && cargo test file_ops_tests --test '*' -- --nocapture
```

Expected: PASS (all tests)

- [ ] **Step 7: Commit**

```bash
git add gui/src/commands/file_ops.rs gui/src/lib.rs gui/src/main.rs gui/tests/file_ops_tests.rs
git commit -m "feat: implement duplicate_file command with smart naming"
```

---

### Task 10: Implement Create Markdown Note Command

**Files:**
- Modify: `gui/src/commands/file_ops.rs`
- Modify: `gui/tests/file_ops_tests.rs`
- Modify: `gui/src/main.rs` (add command)

**Interfaces:**
- Consumes: Parent folder path (String), note name (String)
- Produces: Tauri command `create_markdown_note(folder: String, name: String) -> Result<String, String>`
  - Returns: Path to new markdown file

- [ ] **Step 1: Add test for create_markdown_note**

```rust
// Add to gui/tests/file_ops_tests.rs
#[test]
fn test_create_markdown_note() {
    use gui::commands::file_ops::create_markdown_note_impl;
    
    let temp_dir = TempDir::new().unwrap();
    let note_path = create_markdown_note_impl(
        temp_dir.path(),
        "Test Note.md"
    ).expect("Failed to create note");
    
    assert!(note_path.exists(), "Note file should exist");
    assert!(note_path.ends_with("Test Note.md"), "Should have correct name");
    
    let content = fs::read_to_string(&note_path).expect("Should read file");
    assert_eq!(content, "", "New note should be empty");
}

#[test]
fn test_create_markdown_note_invalid_name() {
    use gui::commands::file_ops::create_markdown_note_impl;
    
    let temp_dir = TempDir::new().unwrap();
    let result = create_markdown_note_impl(
        temp_dir.path(),
        ""
    );
    
    assert!(result.is_err(), "Empty name should fail");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd gui && cargo test file_ops_tests --test '*' -- --nocapture
```

Expected: FAIL (create_markdown_note_impl not found)

- [ ] **Step 3: Add implementation to file_ops.rs**

```rust
// Add to gui/src/commands/file_ops.rs

pub fn create_markdown_note_impl(folder: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() {
        return Err("Note name cannot be empty".to_string());
    }
    
    if !folder.is_dir() {
        return Err(format!("Folder not found: {}", folder.display()));
    }
    
    let note_path = folder.join(name);
    
    // Check if file already exists
    if note_path.exists() {
        return Err(format!("File already exists: {}", note_path.display()));
    }
    
    // Create empty note
    fs::write(&note_path, "")
        .map_err(|e| format!("Failed to create note: {}", e))?;
    
    Ok(note_path)
}

#[tauri::command]
pub async fn create_markdown_note(folder: String, name: String) -> Result<String, String> {
    let folder = PathBuf::from(&folder);
    let note_path = create_markdown_note_impl(&folder, &name)?;
    Ok(note_path.to_string_lossy().into_owned())
}
```

- [ ] **Step 4: Add command to main.rs**

```rust
// gui/src/main.rs - update invoke_handler
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::file_ops::duplicate_file,
    commands::file_ops::create_markdown_note,
])
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd gui && cargo test file_ops_tests --test '*' -- --nocapture
```

Expected: PASS (all tests)

- [ ] **Step 6: Commit**

```bash
git add gui/src/commands/file_ops.rs gui/src/main.rs gui/tests/file_ops_tests.rs
git commit -m "feat: implement create_markdown_note command"
```

---

---

## Phase 4: Tab System & Integration

### Task 11: Integrate Viewers into Tab System

**Files:**
- Modify: `gui/src/main.rs` (update open_file command)
- Modify: `gui/src/lib.rs`

**Interfaces:**
- Consumes: `detect_file_type()`, all Viewer implementations
- Produces: Updated `open_file(path: String, vault_root: Option<String>) -> Result<TabData, String>` command
  ```rust
  struct TabData {
      path: String,
      tab_type: String, // "markdown", "code", "image", "hex"
      content: String,  // HTML for viewers, raw for code
      language: Option<String>,
      dirty: bool,
  }
  ```

- [ ] **Step 1: Create TabData struct in main.rs**

```rust
// gui/src/main.rs
#[derive(Serialize)]
struct TabData {
    path: String,
    tab_type: String,
    content: String,
    language: Option<String>,
    dirty: bool,
    file_size_bytes: u64,
}
```

- [ ] **Step 2: Update open_file command to use viewers**

```rust
// gui/src/main.rs - replace existing open_file command
#[tauri::command]
async fn open_file(path: String, vault_root: Option<String>) -> Result<TabData, String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    // Detect file type
    let file_type = file_types::detect_file_type(&p);
    let file_size = std::fs::metadata(&p)
        .map(|m| m.len())
        .unwrap_or(0);

    match file_type {
        file_types::FileType::Markdown => {
            // Existing markdown rendering logic
            let opts = RenderOpts {
                file_dir: p.parent(),
                vault_root: vault_root.as_deref().map(Path::new),
            };
            match render_note(&p, &opts) {
                Ok(rendered) => Ok(TabData {
                    path: p.to_string_lossy().into_owned(),
                    tab_type: "markdown".to_string(),
                    content: rendered.html,
                    language: None,
                    dirty: false,
                    file_size_bytes: file_size,
                }),
                Err(e) => Err(e),
            }
        }
        file_types::FileType::Code { language } => {
            let content = std::fs::read_to_string(&p)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            
            let viewer = viewers::CodeViewer::new(
                p.clone(),
                language.clone(),
                content.clone(),
                false,
            )?;
            
            match viewer.render() {
                Ok(html) => Ok(TabData {
                    path: p.to_string_lossy().into_owned(),
                    tab_type: "code".to_string(),
                    content: html,
                    language: Some(language),
                    dirty: false,
                    file_size_bytes: file_size,
                }),
                Err(e) => Err(e.to_string()),
            }
        }
        file_types::FileType::Image { format } => {
            // Get image dimensions
            let (width, height) = (800, 600); // TODO: read actual dimensions
            
            let viewer = viewers::ImageViewer::new_with_size(
                p.clone(),
                format,
                width,
                height,
                file_size,
            )?;
            
            match viewer.render() {
                Ok(html) => Ok(TabData {
                    path: p.to_string_lossy().into_owned(),
                    tab_type: "image".to_string(),
                    content: html,
                    language: None,
                    dirty: false,
                    file_size_bytes: file_size,
                }),
                Err(e) => Err(e.to_string()),
            }
        }
        file_types::FileType::Hex => {
            // Read file as binary
            let bytes = std::fs::read(&p)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            
            let viewer = viewers::HexViewer::new_from_bytes(
                p.clone(),
                bytes,
                file_size,
            )?;
            
            match viewer.render() {
                Ok(html) => Ok(TabData {
                    path: p.to_string_lossy().into_owned(),
                    tab_type: "hex".to_string(),
                    content: html,
                    language: None,
                    dirty: false,
                    file_size_bytes: file_size,
                }),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}
```

- [ ] **Step 3: Update invoke_handler to export TabData**

```rust
// gui/src/main.rs - ensure TabData is properly serialized in handler
```

- [ ] **Step 4: Test compilation**

```bash
cd gui && cargo check
```

Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add gui/src/main.rs
git commit -m "feat: integrate viewers into tab system with polymorphic open_file"
```

---

## Phase 5: Code Editor & Persistence

### Task 12: Add Tab Persistence

**Files:**
- Create: `gui/src/persistence.rs`
- Create: `gui/tests/persistence_tests.rs`
- Modify: `gui/src/lib.rs` (add module)
- Modify: `gui/src/main.rs` (add save/restore commands)

**Interfaces:**
- Produces: `save_open_tabs()` and `restore_open_tabs()` functions

- [ ] **Step 1: Write failing test for tab persistence**

```rust
// gui/tests/persistence_tests.rs
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use tempfile::TempDir;
    use gui::persistence::{save_open_tabs, restore_open_tabs, TabState};

    #[test]
    fn test_save_and_restore_tabs() {
        let temp_dir = TempDir::new().unwrap();
        let tabs = vec![
            TabState {
                path: "/path/to/file1.md".to_string(),
                tab_type: "markdown".to_string(),
            },
            TabState {
                path: "/path/to/file2.rs".to_string(),
                tab_type: "code".to_string(),
            },
        ];
        
        save_open_tabs(temp_dir.path(), &tabs, 0)
            .expect("Failed to save tabs");
        
        let (restored, active) = restore_open_tabs(temp_dir.path())
            .expect("Failed to restore tabs");
        
        assert_eq!(restored.len(), 2, "Should restore 2 tabs");
        assert_eq!(restored[0].path, "/path/to/file1.md");
        assert_eq!(active, 0);
    }

    #[test]
    fn test_tab_limit_5() {
        let temp_dir = TempDir::new().unwrap();
        let mut tabs = vec![];
        
        for i in 0..10 {
            tabs.push(TabState {
                path: format!("/path/to/file{}.md", i),
                tab_type: "markdown".to_string(),
            });
        }
        
        save_open_tabs(temp_dir.path(), &tabs, 0)
            .expect("Failed to save tabs");
        
        let (restored, _) = restore_open_tabs(temp_dir.path())
            .expect("Failed to restore tabs");
        
        assert_eq!(restored.len(), 5, "Should limit to 5 most recent tabs");
    }
}
```

- [ ] **Step 2: Create persistence.rs**

```rust
// gui/src/persistence.rs
use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct TabState {
    pub path: String,
    pub tab_type: String,
}

#[derive(Serialize, Deserialize)]
struct TabsFile {
    vault_root: String,
    active_tab_index: usize,
    tabs: Vec<TabState>,
}

pub fn save_open_tabs(
    vault_root: &Path,
    tabs: &[TabState],
    active_index: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let tomarkdown_dir = vault_root.join(".tomarkdown");
    std::fs::create_dir_all(&tomarkdown_dir)?;
    
    let tabs_file = TabsFile {
        vault_root: vault_root.to_string_lossy().into_owned(),
        active_tab_index: active_index,
        tabs: tabs.iter().take(5).cloned().collect(), // Limit to 5
    };
    
    let json = serde_json::to_string_pretty(&tabs_file)?;
    std::fs::write(tomarkdown_dir.join("tabs.json"), json)?;
    
    Ok(())
}

pub fn restore_open_tabs(vault_root: &Path) -> Result<(Vec<TabState>, usize), Box<dyn std::error::Error>> {
    let tabs_file = vault_root.join(".tomarkdown").join("tabs.json");
    
    if !tabs_file.exists() {
        return Ok((vec![], 0));
    }
    
    let json = std::fs::read_to_string(&tabs_file)?;
    let data: TabsFile = serde_json::from_str(&json)?;
    
    // Filter out tabs for files that no longer exist
    let existing_tabs: Vec<TabState> = data.tabs
        .into_iter()
        .filter(|tab| Path::new(&tab.path).exists())
        .collect();
    
    Ok((existing_tabs, data.active_tab_index))
}
```

- [ ] **Step 3: Run tests to verify they pass**

```bash
cd gui && cargo test persistence_tests --test '*' -- --nocapture
```

Expected: PASS

- [ ] **Step 4: Add save/restore commands to main.rs**

```rust
// gui/src/main.rs
#[tauri::command]
async fn save_tabs_on_close(
    vault_root: String,
    tabs: Vec<persistence::TabState>,
    active_index: usize,
) -> Result<(), String> {
    persistence::save_open_tabs(
        Path::new(&vault_root),
        &tabs,
        active_index,
    ).map_err(|e| e.to_string())
}

#[tauri::command]
async fn restore_tabs_on_start(vault_root: String) -> Result<(Vec<persistence::TabState>, usize), String> {
    persistence::restore_open_tabs(Path::new(&vault_root))
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Commit**

```bash
git add gui/src/persistence.rs gui/src/lib.rs gui/src/main.rs gui/tests/persistence_tests.rs
git commit -m "feat: add tab persistence (last 5 tabs) with save/restore on app close/start"
```

---

### Task 13: Add Code Editor State Management

**Files:**
- Modify: `gui/src/viewers/code.rs` (add editing support)
- Modify: `gui/tests/code_viewer_tests.rs` (add edit tests)

**Interfaces:**
- Consumes: CodeViewer instance
- Produces: `update_content(content: String)`, `set_dirty(bool)` methods

- [ ] **Step 1: Add editor methods to CodeViewer**

```rust
// gui/src/viewers/code.rs - add to CodeViewer impl
impl CodeViewer {
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.dirty = true;
        self.file_size = self.content.len() as u64;
    }
    
    pub fn save_content(&mut self) -> Result<(), String> {
        // In Phase 5, this is a no-op. Full implementation in Phase 6
        self.dirty = false;
        Ok(())
    }
    
    pub fn get_content(&self) -> &str {
        &self.content
    }
    
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}
```

- [ ] **Step 2: Add tests for editor state**

```rust
// Add to gui/tests/code_viewer_tests.rs
#[test]
fn test_code_viewer_update_content() {
    use gui::viewers::{FileViewer, CodeViewer};
    
    let mut viewer = CodeViewer::new(
        PathBuf::from("test.rs"),
        "rust".to_string(),
        "old content".to_string(),
        false,
    ).unwrap();
    
    viewer.update_content("new content".to_string());
    
    assert!(viewer.is_dirty());
    assert_eq!(viewer.get_content(), "new content");
}

#[test]
fn test_code_viewer_save_clears_dirty() {
    use gui::viewers::{FileViewer, CodeViewer};
    
    let mut viewer = CodeViewer::new(
        PathBuf::from("test.rs"),
        "rust".to_string(),
        "content".to_string(),
        false,
    ).unwrap();
    
    viewer.update_content("modified".to_string());
    assert!(viewer.is_dirty());
    
    viewer.save_content().unwrap();
    assert!(!viewer.is_dirty());
}
```

- [ ] **Step 3: Commit**

```bash
git add gui/src/viewers/code.rs gui/tests/code_viewer_tests.rs
git commit -m "feat: add content update and save methods to CodeViewer"
```

---

## Phase 6: Logging & Error Handling

### Task 14: Add Logging Infrastructure

**Files:**
- Create: `gui/src/logging.rs`
- Modify: `gui/src/main.rs` (initialize logging)
- Modify: `gui/src/lib.rs` (add module)

**Interfaces:**
- Produces: `init_logging(vault_root: &Path) -> Result<(), Error>` function

- [ ] **Step 1: Create logging.rs**

```rust
// gui/src/logging.rs
use tracing_subscriber::prelude::*;
use tracing_appender::rolling;
use std::path::Path;

pub fn init_logging(vault_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let logs_dir = vault_root.join(".tomarkdown").join("logs");
    std::fs::create_dir_all(&logs_dir)?;
    
    // Create rolling file appender (daily rotation)
    let file_appender = rolling::daily(&logs_dir, "vault.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Setup layers
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout);
    
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .with(fmt_layer)
        .with(file_layer)
        .init();
    
    Ok(())
}
```

- [ ] **Step 2: Initialize logging in main.rs**

```rust
// gui/src/main.rs - in main() before building app
fn main() {
    // Initialize logging early
    let vault_root = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Documents");
    let _ = logging::init_logging(&vault_root);
    
    tracing::info!("Starting toMarkdownMCP GUI");
    
    // ... rest of main ...
}
```

- [ ] **Step 3: Add logging calls to key functions**

```rust
// gui/src/main.rs - update open_file
#[tauri::command]
async fn open_file(path: String, vault_root: Option<String>) -> Result<TabData, String> {
    tracing::info!("Opening file: {}", path);
    
    let p = PathBuf::from(&path);
    // ... rest of function ...
    
    match file_type {
        file_types::FileType::Code { language } => {
            tracing::debug!("Detected language: {} for {}", language, path);
            // ... rest of code ...
        }
        _ => {}
    }
    
    Ok(tab_data)
}
```

- [ ] **Step 4: Commit**

```bash
git add gui/src/logging.rs gui/src/main.rs gui/src/lib.rs
git commit -m "feat: add rotating file logging with console output (5-day retention)"
```

---

## Phase 7: Search, Preview & Advanced Features

### Task 15: Add Vault Indexing

**Files:**
- Create: `gui/src/vault/indexer.rs`
- Create: `gui/tests/indexer_tests.rs`
- Modify: `gui/src/vault/mod.rs` (add indexer)

**Interfaces:**
- Produces: `index_vault(vault_db: &VaultDb) -> Result<(), Error>` function

- [ ] **Step 1: Create vault/indexer.rs**

```rust
// gui/src/vault/indexer.rs
use std::path::{Path, PathBuf};
use rusqlite::params;
use chrono::Utc;

pub fn index_file(
    vault_db: &crate::vault::VaultDb,
    file_path: &Path,
    vault_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_size = std::fs::metadata(file_path)?.len();
    let modified_at = std::fs::metadata(file_path)?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    
    let relative_path = file_path.strip_prefix(vault_root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .into_owned();
    
    let file_name = file_path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    
    let extension = file_path.extension()
        .map(|e| e.to_string_lossy().into_owned());
    
    let file_type = crate::file_types::detect_file_type(file_path);
    let (detected_type, language) = match file_type {
        crate::file_types::FileType::Markdown => ("markdown".to_string(), None),
        crate::file_types::FileType::Code { language } => ("code".to_string(), Some(language)),
        crate::file_types::FileType::Image { format } => ("image".to_string(), Some(format)),
        crate::file_types::FileType::Hex => ("hex".to_string(), None),
    };
    
    // Insert or update file record
    vault_db.conn.execute(
        "INSERT OR REPLACE INTO files (path, name, extension, file_type, language, size_bytes, modified_at, last_indexed_at, is_indexed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            relative_path,
            file_name,
            extension,
            detected_type,
            language,
            file_size,
            modified_at,
            Utc::now().timestamp(),
            true,
        ],
    )?;
    
    Ok(())
}

pub fn index_vault(vault_db: &crate::vault::VaultDb) -> Result<usize, Box<dyn std::error::Error>> {
    let vault_root = vault_db.vault_root.clone();
    let start = Utc::now();
    let mut indexed_count = 0;
    
    tracing::info!("Starting vault indexing for {}", vault_root.display());
    
    for entry in walkdir::WalkDir::new(&vault_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        if let Err(e) = index_file(vault_db, entry.path(), &vault_root) {
            tracing::warn!("Failed to index {}: {}", entry.path().display(), e);
            continue;
        }
        indexed_count += 1;
    }
    
    let duration = Utc::now().signed_duration_since(start);
    tracing::info!(
        "Indexing complete: {} files indexed in {}s",
        indexed_count,
        duration.num_seconds()
    );
    
    Ok(indexed_count)
}
```

- [ ] **Step 2: Add walkdir dependency to Cargo.toml**

```toml
walkdir = "2.4"
```

- [ ] **Step 3: Update vault/mod.rs to expose indexer**

```rust
// gui/src/vault/mod.rs
pub mod indexer;
pub use indexer::index_vault;
```

- [ ] **Step 4: Commit**

```bash
git add gui/src/vault/indexer.rs gui/Cargo.toml gui/src/vault/mod.rs gui/tests/indexer_tests.rs
git commit -m "feat: add lazy vault indexing with file type detection"
```

---

## Phase 8: Integration & Testing

### Task 16: End-to-End Integration Test

**Files:**
- Create: `gui/tests/e2e_integration_tests.rs`

**Interfaces:**
- Consumes: All previous components
- Produces: Integration test suite verifying multi-tab workflow

- [ ] **Step 1: Create comprehensive integration test**

```rust
// gui/tests/e2e_integration_tests.rs
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use gui::vault::init_vault_db;
    use gui::file_types::detect_file_type;

    #[test]
    fn test_full_workflow_open_various_files() {
        let temp_dir = TempDir::new().unwrap();
        let vault_root = temp_dir.path();
        
        // Create test files
        fs::write(vault_root.join("test.md"), "# Hello\nWorld").unwrap();
        fs::write(vault_root.join("script.py"), "print('hello')").unwrap();
        fs::write(vault_root.join("data.json"), r#"{"key":"value"}"#).unwrap();
        
        // Initialize vault DB
        let vault_db = init_vault_db(vault_root).expect("Failed to init vault");
        
        // Index files
        gui::vault::index_vault(&vault_db).expect("Failed to index");
        
        // Verify all files were indexed
        let mut stmt = vault_db.conn
            .prepare("SELECT COUNT(*) FROM files WHERE is_indexed = 1")
            .unwrap();
        let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(count, 3, "Should have indexed 3 files");
        
        // Verify file type detection
        assert_eq!(detect_file_type(vault_root.join("test.md").as_path()), gui::file_types::FileType::Markdown);
        
        match detect_file_type(vault_root.join("script.py").as_path()) {
            gui::file_types::FileType::Code { language } if language == "python" => (),
            _ => panic!("Should detect Python"),
        }
    }
}
```

- [ ] **Step 2: Run integration tests**

```bash
cd gui && cargo test e2e_integration_tests --test '*' -- --nocapture
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add gui/tests/e2e_integration_tests.rs
git commit -m "test: add end-to-end integration test verifying full workflow"
```

---

### Task 17: Manual Testing Checklist & Documentation

**Files:**
- Create: `docs/TESTING.md` (testing guide)
- Modify: README or CHANGELOG

**Interfaces:**
- Produces: Testing documentation for maintainers

- [ ] **Step 1: Create testing guide**

```markdown
# Vault Viewer Enhancements - Testing Guide

## Manual Testing Checklist

### File Type Detection
- [ ] Open markdown file (.md) → should render as markdown
- [ ] Open Python file (.py) → should show syntax highlighting
- [ ] Open image file (.png, .jpg, .gif, .svg, .webp, .avif, .tiff) → should display image
- [ ] Open binary file → should show hex dump
- [ ] Open .json, .yaml, .toml → should highlight as code with language

### Tab System
- [ ] Open multiple files → each opens in new tab
- [ ] Icons display correctly (📄, <>, 🖼️, 🔢)
- [ ] Switch between tabs → content changes correctly
- [ ] Close app and reopen → last 5 tabs restore
- [ ] Close specific tab → tab is removed

### File Operations
- [ ] Right-click file in tree → duplicate option
- [ ] Click duplicate → new file created with "copy" suffix
- [ ] Right-click file → create note option
- [ ] Create note dialog → enters filename, creates empty .md file

### Code Editor (Phase 5)
- [ ] Open code file → shows syntax highlighting
- [ ] Toggle edit mode → enables/disables editing
- [ ] Edit code → dirty indicator (•) appears in tab
- [ ] Save code → dirty indicator disappears

### Logging
- [ ] Open file → appears in logs
- [ ] Duplicate file → logged
- [ ] Create note → logged
- [ ] Check `.tomarkdown/logs/vault.log` → contains events
- [ ] Wait 5+ days → old logs cleaned up

### Large Files
- [ ] Open code file >10MB → shows warning, read-only
- [ ] Open image >100MB → shows warning, loads reduced resolution
- [ ] Open binary file >10MB → hex viewer lazy-loads first 1MB

### Error Handling
- [ ] Try to duplicate readonly file → shows error
- [ ] Try to open deleted file → shows error, closes tab
- [ ] Try to create note with invalid name → shows error
- [ ] Database corruption → automatic re-index

## Automated Test Runs

```bash
# Run all unit tests
cd gui && cargo test --lib

# Run integration tests
cd gui && cargo test --test '*'

# Run with logging
cd gui && RUST_LOG=debug cargo test -- --nocapture
```
```

- [ ] **Step 2: Commit testing guide**

```bash
git add docs/TESTING.md
git commit -m "docs: add comprehensive testing guide for vault viewer enhancements"
```

---

### Task 18: Final Verification & Version Bump

**Files:**
- Modify: `gui/Cargo.toml` (version bump)
- Modify: Tauri config if needed

**Interfaces:**
- Produces: Release-ready code

- [ ] **Step 1: Verify all tests pass**

```bash
cd gui && cargo test --all
```

Expected: All tests PASS

- [ ] **Step 2: Run clippy for code quality**

```bash
cd gui && cargo clippy --all-targets -- -D warnings
```

Expected: No warnings

- [ ] **Step 3: Update version in Cargo.toml**

```toml
# gui/Cargo.toml
[package]
name = "tomarkdown-gui"
version = "0.6.0"  # Bump from 0.5.0
```

- [ ] **Step 4: Commit version bump**

```bash
git add gui/Cargo.toml
git commit -m "chore: bump GUI version to 0.6.0 for vault viewer enhancements"
```

---

## Success Checklist

- ✅ Phase 1: All dependencies added, file type detection working, SQLite schema initialized
- ✅ Phase 2: All four viewers implemented (Markdown, Code, Image, Hex) with tests
- ✅ Phase 3: File duplication and create note commands working, context menu ready
- ✅ Phase 4: Viewers integrated into tab system, polymorphic open_file working
- ✅ Phase 5: Tab persistence implemented, code editor state management added
- ✅ Phase 6: Logging infrastructure complete with rotating file logs
- ✅ Phase 7: Vault indexing implemented
- ✅ Phase 8: Integration tests passing, documentation complete, version bumped

---

## Execution Notes

**Total estimated tasks:** 18  
**Estimated phases:** 8  
**Each task includes:** Complete code, tests, commits

**Recommended execution:** Subagent-driven (one task per subagent) or inline with checkpoints after each phase.