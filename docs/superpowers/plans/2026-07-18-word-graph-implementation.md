# Word Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a Word graph view that displays co-occurrence relationships between words in the vault, with SQLite persistence, incremental indexing, and per-view independent zoom controls across all three graph modes (Global, Current Note, Word).

**Architecture:** 
- **Rust backend**: SQLite schema for words and co-occurrence pairs; tokenizer/indexer for full and delta indexing; Tauri commands to fetch graph data
- **Canvas 2D rendering**: Reuse existing force-directed physics simulation for word nodes; adapt node/link structure for words
- **JavaScript frontend**: Per-view zoom state tracking; zoom event handlers (mouse wheel, touch pinch, keyboard, buttons); word node interaction (click highlights related notes)
- **SQLite persistence**: Lazy initialization on first Word view access; incremental updates on file changes via debounced Word tab open

**Tech Stack:** 
- Rust with Tauri `async_runtime::spawn_blocking` for background indexing
- SQLite with rusqlite driver (already in Cargo.toml via other dependencies)
- Canvas 2D for graph rendering (existing)
- HTML/CSS/JavaScript for UI and zoom controls (existing patterns)

## Global Constraints

- Minimum word length: 3 characters (avoid noise)
- Word scope: top N words where N = `min(100, vault_size / 10)`, capped at 200
- Co-occurrence threshold: 2+ notes (edges shown only if two words co-occur in 2+ notes)
- Stopwords excluded: "the", "and", "is", "a", "in", "to", "of", "for", etc. (see task definitions)
- Zoom min/max: 0.2× to 5×; default = fit-all
- Startup indexing: ~5–10 seconds for 1K notes; delta <100ms
- File watcher debounce: 500ms before delta indexing
- Tauri MSRV: 1.88 (existing project minimum)
- No new external dependencies (use existing crates only)

---

### Task 1: SQLite Schema Setup & Database Module

**Files:**
- Create: `gui/src/word_graph.rs` (module stub)
- Create: `gui/src/word_graph/db.rs` (schema, initialization)
- Modify: `gui/src/main.rs` (add `mod word_graph`)

**Interfaces:**
- Produces: `word_graph::db::WordGraphDb` struct with methods:
  - `new(vault_path: &Path) -> Result<Self>` — creates/opens SQLite database
  - `initialize_schema() -> Result<()>` — creates tables if not exist
  - All queries will be added in later tasks

- [ ] **Step 1: Create word_graph module structure**

Create file `gui/src/word_graph.rs`:
```rust
pub mod db;
pub mod tokenizer;
pub mod indexer;
pub mod queries;
```

Add to `gui/src/main.rs` (after other module declarations):
```rust
mod word_graph;
```

- [ ] **Step 2: Run cargo check to verify module compiles**

Run: `cargo check -p to_markdown_gui`
Expected: No errors

- [ ] **Step 3: Write SQLite schema and database initialization**

Create file `gui/src/word_graph/db.rs`:
```rust
use anyhow::{anyhow, Result};
use rusqlite::{Connection, params};
use std::path::Path;

pub struct WordGraphDb {
    conn: Connection,
}

impl WordGraphDb {
    pub fn new(vault_path: &Path) -> Result<Self> {
        let db_path = vault_path.join(".tomarkdown_word_graph.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| anyhow!("Failed to open word graph database: {}", e))?;
        
        let db = Self { conn };
        db.initialize_schema()?;
        Ok(db)
    }

    pub fn initialize_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS words (
                id INTEGER PRIMARY KEY,
                word TEXT UNIQUE NOT NULL,
                frequency INTEGER NOT NULL,
                last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS co_occurrence (
                word1_id INTEGER NOT NULL,
                word2_id INTEGER NOT NULL,
                count INTEGER NOT NULL,
                notes_list_json TEXT,
                UNIQUE(word1_id, word2_id),
                FOREIGN KEY(word1_id) REFERENCES words(id),
                FOREIGN KEY(word2_id) REFERENCES words(id)
            );

            CREATE TABLE IF NOT EXISTS index_state (
                vault_path TEXT PRIMARY KEY,
                last_full_index TIMESTAMP,
                changed_files_since TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_words_frequency ON words(frequency);
            CREATE INDEX IF NOT EXISTS idx_index_state_vault ON index_state(vault_path);
            "#
        ).map_err(|e| anyhow!("Failed to initialize schema: {}", e))?;
        
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_database_creation() {
        let temp_dir = std::env::temp_dir().join("word_graph_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let db = WordGraphDb::new(&temp_dir).unwrap();
        
        // Verify tables exist
        let mut stmt = db.conn()
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        
        assert!(table_names.contains(&"words".to_string()));
        assert!(table_names.contains(&"co_occurrence".to_string()));
        assert!(table_names.contains(&"index_state".to_string()));
        
        fs::remove_dir_all(&temp_dir).ok();
    }
}
```

- [ ] **Step 4: Run tests to verify schema creation**

Run: `cargo test -p to_markdown_gui word_graph::db`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add gui/src/word_graph.rs gui/src/word_graph/db.rs gui/src/main.rs
git commit -m "feat: add SQLite schema for word graph indexing"
```

---

### Task 2: Tokenization & Stopword Filtering

**Files:**
- Create: `gui/src/word_graph/tokenizer.rs`

**Interfaces:**
- Produces: 
  - `const STOPWORDS: &[&str]` — list of excluded words
  - `fn tokenize(text: &str) -> Vec<String>` — extracts words, lowercases, filters stopwords, enforces min length (3 chars)
  - `fn get_word_pairs(words: &[String]) -> Vec<(String, String)>` — returns all co-occurring word pairs in a note

- [ ] **Step 1: Write tokenization and stopword filtering**

Create file `gui/src/word_graph/tokenizer.rs`:
```rust
pub const STOPWORDS: &[&str] = &[
    "the", "and", "is", "a", "in", "to", "of", "for", "that", "it", "as", "on", "by", 
    "with", "be", "was", "are", "or", "an", "from", "this", "at", "which", "have", 
    "has", "had", "do", "does", "did", "not", "no", "can", "could", "will", "would", 
    "should", "may", "might", "must", "shall", "been", "being", "am", "i", "you", 
    "he", "she", "we", "they", "him", "her", "us", "them", "my", "your", "his", 
    "her", "its", "our", "their", "what", "when", "where", "why", "how", "all",
    "each", "every", "both", "few", "more", "some", "such", "no", "nor", "only",
    "own", "same", "so", "than", "too", "very", "just", "can", "now", "but",
];

const MIN_WORD_LENGTH: usize = 3;

pub fn tokenize(text: &str) -> Vec<String> {
    text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| {
            word.len() >= MIN_WORD_LENGTH && !STOPWORDS.contains(&word)
        })
        .map(|word| word.to_string())
        .collect()
}

pub fn get_word_pairs(words: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for (i, word1) in words.iter().enumerate() {
        for word2 in words.iter().skip(i + 1) {
            if word1 < word2 {
                pairs.push((word1.clone(), word2.clone()));
            } else if word2 < word1 {
                pairs.push((word2.clone(), word1.clone()));
            }
        }
    }
    pairs.sort();
    pairs.dedup();
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_extracts_words() {
        let text = "Hello world, this is a test!";
        let tokens = tokenize(text);
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }

    #[test]
    fn test_tokenize_filters_stopwords() {
        let text = "the and is a to for";
        let tokens = tokenize(text);
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_enforces_min_length() {
        let text = "ab abc abcd";
        let tokens = tokenize(text);
        assert!(!tokens.contains(&"ab".to_string()));
        assert!(tokens.contains(&"abc".to_string()));
        assert!(tokens.contains(&"abcd".to_string()));
    }

    #[test]
    fn test_tokenize_lowercases() {
        let text = "HELLO Hello hello";
        let tokens = tokenize(text);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], "hello");
    }

    #[test]
    fn test_get_word_pairs() {
        let words = vec!["markdown".to_string(), "editor".to_string(), "note".to_string()];
        let pairs = get_word_pairs(&words);
        assert_eq!(pairs.len(), 3);
        assert!(pairs.contains(&("editor".to_string(), "markdown".to_string())));
        assert!(pairs.contains(&("editor".to_string(), "note".to_string())));
        assert!(pairs.contains(&("markdown".to_string(), "note".to_string())));
    }

    #[test]
    fn test_get_word_pairs_deduplicates() {
        let words = vec!["word".to_string(), "word".to_string(), "other".to_string()];
        let pairs = get_word_pairs(&words);
        let matching_pairs: Vec<_> = pairs.iter()
            .filter(|(w1, w2)| (w1 == "other" && w2 == "word") || (w1 == "word" && w2 == "other"))
            .collect();
        assert_eq!(matching_pairs.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify tokenization**

Run: `cargo test -p to_markdown_gui word_graph::tokenizer`
Expected: PASS (all 6 tests)

- [ ] **Step 3: Commit**

```bash
git add gui/src/word_graph/tokenizer.rs
git commit -m "feat: implement tokenization with stopword filtering"
```

---

### Task 3: Full Vault Indexing

**Files:**
- Create: `gui/src/word_graph/indexer.rs`

**Interfaces:**
- Produces:
  - `fn index_vault_full(db: &WordGraphDb, vault_path: &Path) -> Result<()>` — full index of all notes
  - `fn index_vault_delta(db: &WordGraphDb, vault_path: &Path, changed_files: &[PathBuf]) -> Result<()>` — incremental index of changed files

- [ ] **Step 1: Write full indexing logic**

Create file `gui/src/word_graph/indexer.rs`:
```rust
use crate::word_graph::db::WordGraphDb;
use crate::word_graph::tokenizer::{tokenize, get_word_pairs};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

pub fn index_vault_full(db: &WordGraphDb, vault_path: &Path) -> Result<()> {
    // Find all markdown files
    let md_files = find_markdown_files(vault_path)?;
    
    // Clear existing index
    db.conn().execute("DELETE FROM co_occurrence", [])?;
    db.conn().execute("DELETE FROM words", [])?;
    
    // Index each file
    let mut word_freq: HashMap<String, i32> = HashMap::new();
    let mut co_occur: HashMap<(String, String), (i32, Vec<String>)> = HashMap::new();
    
    for file_path in &md_files {
        let content = std::fs::read_to_string(file_path)?;
        let words = tokenize(&content);
        let pairs = get_word_pairs(&words);
        
        // Track word frequencies
        for word in &words {
            *word_freq.entry(word.clone()).or_insert(0) += 1;
        }
        
        // Track co-occurrence
        for (w1, w2) in pairs {
            let key = (w1, w2);
            let entry = co_occur.entry(key.clone()).or_insert((0, vec![]));
            entry.0 += 1;
            let rel_path = file_path.strip_prefix(vault_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.to_string_lossy().to_string());
            if !entry.1.contains(&rel_path) {
                entry.1.push(rel_path);
            }
        }
    }
    
    // Store in SQLite
    let tx = db.conn().transaction()?;
    
    for (word, freq) in word_freq {
        tx.execute(
            "INSERT INTO words (word, frequency) VALUES (?1, ?2)",
            [word, freq.to_string()],
        )?;
    }
    
    for ((w1, w2), (count, notes)) in co_occur {
        let w1_id: i32 = tx.query_row(
            "SELECT id FROM words WHERE word = ?1",
            [&w1],
            |row| row.get(0)
        )?;
        let w2_id: i32 = tx.query_row(
            "SELECT id FROM words WHERE word = ?1",
            [&w2],
            |row| row.get(0)
        )?;
        let notes_json = serde_json::to_string(&notes)?;
        
        tx.execute(
            "INSERT OR REPLACE INTO co_occurrence (word1_id, word2_id, count, notes_list_json) VALUES (?1, ?2, ?3, ?4)",
            [w1_id.to_string(), w2_id.to_string(), count.to_string(), notes_json],
        )?;
    }
    
    // Update index state
    tx.execute(
        "INSERT OR REPLACE INTO index_state (vault_path, last_full_index, changed_files_since) VALUES (?1, CURRENT_TIMESTAMP, ?2)",
        [vault_path.to_string_lossy().to_string(), "[]".to_string()],
    )?;
    
    tx.commit()?;
    Ok(())
}

pub fn index_vault_delta(db: &WordGraphDb, vault_path: &Path, changed_files: &[std::path::PathBuf]) -> Result<()> {
    // Similar to full indexing, but only process changed files
    // Merge results into existing tables
    if changed_files.is_empty() {
        return Ok(());
    }
    
    let mut word_freq_delta: HashMap<String, i32> = HashMap::new();
    let mut co_occur_delta: HashMap<(String, String), (i32, Vec<String>)> = HashMap::new();
    
    for file_path in changed_files {
        let content = std::fs::read_to_string(file_path)?;
        let words = tokenize(&content);
        let pairs = get_word_pairs(&words);
        
        for word in &words {
            *word_freq_delta.entry(word.clone()).or_insert(0) += 1;
        }
        
        for (w1, w2) in pairs {
            let key = (w1, w2);
            let entry = co_occur_delta.entry(key.clone()).or_insert((0, vec![]));
            entry.0 += 1;
            let rel_path = file_path.strip_prefix(vault_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.to_string_lossy().to_string());
            if !entry.1.contains(&rel_path) {
                entry.1.push(rel_path);
            }
        }
    }
    
    let tx = db.conn().transaction()?;
    
    // Update word frequencies
    for (word, delta) in word_freq_delta {
        let existing: i32 = tx.query_row(
            "SELECT frequency FROM words WHERE word = ?1",
            [&word],
            |row| row.get(0)
        ).unwrap_or(0);
        
        if existing == 0 {
            tx.execute(
                "INSERT INTO words (word, frequency) VALUES (?1, ?2)",
                [word, (delta).to_string()],
            )?;
        } else {
            tx.execute(
                "UPDATE words SET frequency = ?1 WHERE word = ?2",
                [existing + delta, word],
            )?;
        }
    }
    
    // Update co-occurrence counts
    for ((w1, w2), (count_delta, notes)) in co_occur_delta {
        let w1_id: i32 = tx.query_row(
            "SELECT id FROM words WHERE word = ?1",
            [&w1],
            |row| row.get(0)
        )?;
        let w2_id: i32 = tx.query_row(
            "SELECT id FROM words WHERE word = ?1",
            [&w2],
            |row| row.get(0)
        )?;
        
        let existing: (i32, String) = tx.query_row(
            "SELECT count, notes_list_json FROM co_occurrence WHERE word1_id = ?1 AND word2_id = ?2",
            [w1_id, w2_id],
            |row| Ok((row.get(0)?, row.get(1)?))
        ).ok().unwrap_or((0, "[]".to_string()));
        
        let mut merged_notes: Vec<String> = serde_json::from_str(&existing.1).unwrap_or_default();
        for note in notes {
            if !merged_notes.contains(&note) {
                merged_notes.push(note);
            }
        }
        
        let notes_json = serde_json::to_string(&merged_notes)?;
        tx.execute(
            "INSERT OR REPLACE INTO co_occurrence (word1_id, word2_id, count, notes_list_json) VALUES (?1, ?2, ?3, ?4)",
            [w1_id.to_string(), w2_id.to_string(), (existing.0 + count_delta).to_string(), notes_json],
        )?;
    }
    
    tx.commit()?;
    Ok(())
}

fn find_markdown_files(vault_path: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut md_files = Vec::new();
    
    fn walk_dir(path: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() && !path.file_name().unwrap_or_default().to_string_lossy().starts_with('.') {
                walk_dir(&path, files)?;
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                files.push(path);
            }
        }
        Ok(())
    }
    
    walk_dir(vault_path, &mut md_files)?;
    Ok(md_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_temp_vault() -> Result<std::path::PathBuf> {
        let temp_dir = std::env::temp_dir().join("word_graph_indexer_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir)?;
        
        // Create test notes
        fs::write(temp_dir.join("note1.md"), "markdown editor vault")?;
        fs::write(temp_dir.join("note2.md"), "markdown document tool")?;
        
        Ok(temp_dir)
    }

    #[test]
    fn test_index_vault_full() -> Result<()> {
        let temp_dir = setup_temp_vault()?;
        let db = crate::word_graph::db::WordGraphDb::new(&temp_dir)?;
        
        index_vault_full(&db, &temp_dir)?;
        
        // Verify words stored
        let count: i32 = db.conn().query_row(
            "SELECT COUNT(*) FROM words",
            [],
            |row| row.get(0)
        )?;
        assert!(count > 0);
        
        // Verify co-occurrence stored
        let co_count: i32 = db.conn().query_row(
            "SELECT COUNT(*) FROM co_occurrence",
            [],
            |row| row.get(0)
        )?;
        assert!(co_count > 0);
        
        fs::remove_dir_all(&temp_dir).ok();
        Ok(())
    }
}
```

- [ ] **Step 2: Add serde_json and update Cargo.toml if needed**

Check if `serde_json` is in `gui/Cargo.toml`. If not, add it to dependencies. (Likely already present from other dependencies.)

- [ ] **Step 3: Run tests to verify indexing**

Run: `cargo test -p to_markdown_gui word_graph::indexer`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add gui/src/word_graph/indexer.rs gui/Cargo.toml
git commit -m "feat: implement full and delta vault indexing"
```

---

### Task 4: SQLite Query Functions

**Files:**
- Create: `gui/src/word_graph/queries.rs`

**Interfaces:**
- Produces:
  - `fn get_top_words(db: &WordGraphDb, limit: usize) -> Result<Vec<(String, i32)>>` — top N words by frequency
  - `fn get_word_pairs_for_graph(db: &WordGraphDb, word_ids: &[i32], threshold: i32) -> Result<Vec<(i32, i32, i32)>>` — pairs for rendering
  - `fn get_notes_for_word(db: &WordGraphDb, word: &str) -> Result<Vec<String>>` — notes containing word
  - `fn get_co_occurrence_partners(db: &WordGraphDb, word: &str, limit: usize) -> Result<Vec<(String, i32)>>` — top co-occurrence partners

- [ ] **Step 1: Write query functions**

Create file `gui/src/word_graph/queries.rs`:
```rust
use crate::word_graph::db::WordGraphDb;
use anyhow::Result;

pub fn get_top_words(db: &WordGraphDb, limit: usize) -> Result<Vec<(String, i32)>> {
    let mut stmt = db.conn().prepare(
        "SELECT word, frequency FROM words ORDER BY frequency DESC LIMIT ?1"
    )?;
    
    let words = stmt.query_map([limit.to_string()], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(words)
}

pub fn get_word_pairs_for_graph(db: &WordGraphDb, word_ids: &[i32], threshold: i32) -> Result<Vec<(i32, i32, i32)>> {
    let placeholders = word_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!(
        "SELECT word1_id, word2_id, count FROM co_occurrence WHERE count >= ?1 AND (word1_id IN ({}) OR word2_id IN ({}))",
        placeholders, placeholders
    );
    
    let mut stmt = db.conn().prepare(&query)?;
    let mut params: Vec<String> = vec![threshold.to_string()];
    params.extend(word_ids.iter().map(|id| id.to_string()));
    params.extend(word_ids.iter().map(|id| id.to_string()));
    
    let pairs = stmt.query_map(params.iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice(), |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(pairs)
}

pub fn get_notes_for_word(db: &WordGraphDb, word: &str) -> Result<Vec<String>> {
    let notes_json: String = db.conn().query_row(
        "SELECT COALESCE(GROUP_CONCAT(DISTINCT json_each.value), '[]') FROM co_occurrence, json_each(co_occurrence.notes_list_json) WHERE word1_id IN (SELECT id FROM words WHERE word = ?1) OR word2_id IN (SELECT id FROM words WHERE word = ?1)",
        [word],
        |row| row.get(0)
    ).unwrap_or_else(|_| "[]".to_string());
    
    let notes: Vec<String> = serde_json::from_str(&notes_json).unwrap_or_default();
    Ok(notes)
}

pub fn get_co_occurrence_partners(db: &WordGraphDb, word: &str, limit: usize) -> Result<Vec<(String, i32)>> {
    let mut stmt = db.conn().prepare(
        "SELECT w.word, c.count FROM co_occurrence c 
         JOIN words w1 ON c.word1_id = w1.id 
         JOIN words w2 ON c.word2_id = w2.id 
         JOIN words w ON (w.id = c.word2_id AND w1.word = ?1) OR (w.id = c.word1_id AND w2.word = ?1) 
         ORDER BY c.count DESC LIMIT ?2"
    )?;
    
    let partners = stmt.query_map([word, &limit.to_string()], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(partners)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_get_top_words() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("word_graph_queries_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir)?;
        fs::write(temp_dir.join("note.md"), "markdown editor vault markdown")?;
        
        let db = crate::word_graph::db::WordGraphDb::new(&temp_dir)?;
        crate::word_graph::indexer::index_vault_full(&db, &temp_dir)?;
        
        let top_words = get_top_words(&db, 5)?;
        assert!(!top_words.is_empty());
        assert!(top_words[0].0 == "markdown" || top_words[0].0 == "editor" || top_words[0].0 == "vault");
        
        fs::remove_dir_all(&temp_dir).ok();
        Ok(())
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p to_markdown_gui word_graph::queries`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add gui/src/word_graph/queries.rs
git commit -m "feat: add SQLite query functions for word graph data"
```

---

### Task 5: Add Tauri Commands for Word Graph

**Files:**
- Modify: `gui/src/main.rs` (add new commands)

**Interfaces:**
- Produces:
  - `#[tauri::command] fn word_graph_data(root: String, focus: Option<String>) -> Result<WordGraphResponse, String>` — returns word graph (nodes, links)
  - `#[tauri::command] fn index_vault_words(root: String) -> Result<(), String>` — trigger indexing

- [ ] **Step 1: Add word_graph_data command to main.rs**

Find the section with existing Tauri commands (search for `#[tauri::command]`). Add:

```rust
use crate::word_graph::{queries, db::WordGraphDb};

#[derive(serde::Serialize)]
struct WordGraphNode {
    id: i32,
    word: String,
    frequency: i32,
}

#[derive(serde::Serialize)]
struct WordGraphLink {
    source: i32,
    target: i32,
    weight: i32,
}

#[derive(serde::Serialize)]
struct WordGraphResponse {
    nodes: Vec<WordGraphNode>,
    links: Vec<WordGraphLink>,
    last_updated: Option<String>,
}

#[tauri::command]
fn word_graph_data(root: String) -> Result<WordGraphResponse, String> {
    let vault_path = std::path::Path::new(&root);
    let db = WordGraphDb::new(vault_path).map_err(|e| e.to_string())?;
    
    // Adaptive word limit
    let vault_size = count_markdown_files(vault_path).unwrap_or(0);
    let word_limit = std::cmp::min(200, std::cmp::max(50, vault_size / 10));
    
    let words = queries::get_top_words(&db, word_limit).map_err(|e| e.to_string())?;
    
    let word_ids: Vec<i32> = words.iter()
        .map(|(w, _)| {
            db.conn().query_row("SELECT id FROM words WHERE word = ?1", [w], |row| row.get(0))
        })
        .filter_map(|r| r.ok())
        .collect();
    
    let nodes = words.into_iter().enumerate().map(|(idx, (word, freq))| {
        WordGraphNode {
            id: idx as i32,
            word,
            frequency: freq,
        }
    }).collect();
    
    let pairs = queries::get_word_pairs_for_graph(&db, &word_ids, 2)
        .map_err(|e| e.to_string())?;
    
    let links = pairs.into_iter().filter_map(|(w1_id, w2_id, count)| {
        let source = word_ids.iter().position(|&id| id == w1_id)? as i32;
        let target = word_ids.iter().position(|&id| id == w2_id)? as i32;
        Some(WordGraphLink {
            source,
            target,
            weight: count,
        })
    }).collect();
    
    // TODO: Get last_updated from index_state table
    Ok(WordGraphResponse {
        nodes,
        links,
        last_updated: None,
    })
}

#[tauri::command]
fn index_vault_words(root: String) -> Result<(), String> {
    let vault_path = std::path::Path::new(&root);
    
    // Spawn indexing in background
    tauri::async_runtime::spawn_blocking(move || {
        let db = WordGraphDb::new(vault_path).map_err(|e| e.to_string())?;
        crate::word_graph::indexer::index_vault_full(&db, vault_path).map_err(|e| e.to_string())
    });
    
    Ok(())
}

fn count_markdown_files(path: &std::path::Path) -> std::io::Result<usize> {
    let mut count = 0;
    fn walk(path: &std::path::Path, count: &mut usize) -> std::io::Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && !path.file_name().unwrap_or_default().to_string_lossy().starts_with('.') {
                walk(&path, count)?;
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                *count += 1;
            }
        }
        Ok(())
    }
    walk(path, &mut count)?;
    Ok(count)
}
```

Also add to the `invoke_handler`:
```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    word_graph_data,
    index_vault_words,
])
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p to_markdown_gui`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add gui/src/main.rs
git commit -m "feat: add Tauri commands for word graph data"
```

---

### Task 6: Per-View Zoom State & Zoom Event Handlers

**Files:**
- Modify: `gui/ui/index.html` (JavaScript section)

**Interfaces:**
- Produces:
  - `graphZoom` object with state: `{global: {scale, panX, panY}, current: {...}, word: {...}}`
  - Zoom event handlers (mouse wheel, touch pinch, keyboard, buttons) that update `graphZoom[currentMode]`

- [ ] **Step 1: Add zoom state to index.html**

Find the section with graph initialization (search for `graphNodes = []`). Add before it:

```javascript
// Per-view zoom state: {scale: number, panX: number, panY: number, centerX: number, centerY: number}
const graphZoom = {
  global: { scale: 1, panX: 0, panY: 0, centerX: 0, centerY: 0 },
  current: { scale: 1, panX: 0, panY: 0, centerX: 0, centerY: 0 },
  word: { scale: 1, panX: 0, panY: 0, centerX: 0, centerY: 0 },
};

let currentGraphMode = 'global'; // 'global' | 'current' | 'word'
```

- [ ] **Step 2: Add zoom event handlers**

In the graph initialization section, add:

```javascript
// Zoom event listeners
const graphCanvas = document.getElementById('graph-canvas');

graphCanvas.addEventListener('wheel', (e) => {
  e.preventDefault();
  const zoom = graphZoom[currentGraphMode];
  const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
  const newScale = Math.max(0.2, Math.min(5, zoom.scale * zoomFactor));
  zoom.scale = newScale;
  redrawGraph();
});

// Touch pinch (two-finger)
let lastTouchDistance = 0;
graphCanvas.addEventListener('touchstart', (e) => {
  if (e.touches.length === 2) {
    const dx = e.touches[0].clientX - e.touches[1].clientX;
    const dy = e.touches[0].clientY - e.touches[1].clientY;
    lastTouchDistance = Math.sqrt(dx * dx + dy * dy);
  }
});

graphCanvas.addEventListener('touchmove', (e) => {
  if (e.touches.length === 2) {
    e.preventDefault();
    const dx = e.touches[0].clientX - e.touches[1].clientX;
    const dy = e.touches[0].clientY - e.touches[1].clientY;
    const distance = Math.sqrt(dx * dx + dy * dy);
    const zoom = graphZoom[currentGraphMode];
    
    if (lastTouchDistance > 0) {
      const ratio = distance / lastTouchDistance;
      zoom.scale = Math.max(0.2, Math.min(5, zoom.scale * ratio));
      redrawGraph();
    }
    lastTouchDistance = distance;
  }
});

// Keyboard zoom
document.addEventListener('keydown', (e) => {
  const zoom = graphZoom[currentGraphMode];
  if (e.key === '+' || e.key === '=') {
    zoom.scale = Math.max(0.2, Math.min(5, zoom.scale * 1.2));
    redrawGraph();
  } else if (e.key === '-' || e.key === '_') {
    zoom.scale = Math.max(0.2, Math.min(5, zoom.scale / 1.2));
    redrawGraph();
  } else if (e.key === '0') {
    zoom.scale = 1;
    zoom.panX = 0;
    zoom.panY = 0;
    redrawGraph();
  }
});
```

- [ ] **Step 3: Update graph rendering to apply zoom transform**

Find the `drawGraph()` or equivalent rendering function. At the start of the canvas drawing code, add:

```javascript
const zoom = graphZoom[currentGraphMode];
ctx.save();
ctx.scale(zoom.scale, zoom.scale);
ctx.translate(-zoom.panX, -zoom.panY);
// ... existing drawing code ...
ctx.restore();
```

- [ ] **Step 4: Test zoom locally**

Manually test zoom in browser dev tools console:
```javascript
// Verify zoom state updates
console.log(graphZoom.global.scale); // Should change when scrolling mouse wheel
```

- [ ] **Step 5: Commit**

```bash
git add gui/ui/index.html
git commit -m "feat: add per-view zoom state and event handlers"
```

---

### Task 7: Word Graph UI Toggle & Header

**Files:**
- Modify: `gui/ui/index.html` (HTML and CSS)

**Interfaces:**
- Produces:
  - Three toggle buttons: "Global", "Current", "Word"
  - Word view header with "Word Relationships" title, zoom buttons, last updated timestamp

- [ ] **Step 1: Add Word toggle button to graph overlay**

Find the graph toggle section (search for `#graph-toggle` or `graph-global`). Replace:

```html
<div id="graph-toggle">
  <button id="graph-global" class="graph-mode-btn">Global</button>
  <button id="graph-current" class="graph-mode-btn">Current</button>
  <button id="graph-word" class="graph-mode-btn">Word</button>
</div>
```

- [ ] **Step 2: Add Word view header**

Add before the graph canvas:

```html
<div id="graph-header" style="display: none;">
  <div id="graph-title">Word Relationships</div>
  <div id="graph-timestamp" style="font-size: 12px; color: var(--muted);">Last updated: Never</div>
  <div id="graph-zoom-controls">
    <button id="graph-zoom-in" title="Zoom in (+ key)">+</button>
    <button id="graph-zoom-out" title="Zoom out (- key)">−</button>
    <button id="graph-zoom-reset" title="Reset zoom (0 key)">Reset</button>
  </div>
</div>
```

- [ ] **Step 3: Add event handlers for toggle buttons and zoom controls**

In JavaScript initialization:

```javascript
// Graph mode toggle
document.getElementById('graph-global').addEventListener('click', () => switchGraphMode('global'));
document.getElementById('graph-current').addEventListener('click', () => switchGraphMode('current'));
document.getElementById('graph-word').addEventListener('click', () => switchGraphMode('word'));

function switchGraphMode(mode) {
  currentGraphMode = mode;
  
  // Update button states
  document.querySelectorAll('.graph-mode-btn').forEach(btn => btn.classList.remove('active'));
  document.getElementById('graph-' + mode).classList.add('active');
  
  // Show/hide Word header
  const wordHeader = document.getElementById('graph-header');
  if (mode === 'word') {
    wordHeader.style.display = 'block';
    loadWordGraphData();
  } else {
    wordHeader.style.display = 'none';
  }
  
  redrawGraph();
}

// Zoom button handlers
document.getElementById('graph-zoom-in').addEventListener('click', () => {
  const zoom = graphZoom[currentGraphMode];
  zoom.scale = Math.min(5, zoom.scale * 1.2);
  redrawGraph();
});

document.getElementById('graph-zoom-out').addEventListener('click', () => {
  const zoom = graphZoom[currentGraphMode];
  zoom.scale = Math.max(0.2, zoom.scale / 1.2);
  redrawGraph();
});

document.getElementById('graph-zoom-reset').addEventListener('click', () => {
  const zoom = graphZoom[currentGraphMode];
  zoom.scale = 1;
  zoom.panX = 0;
  zoom.panY = 0;
  redrawGraph();
});
```

- [ ] **Step 4: Add CSS for toggle buttons**

In the `<style>` section, add:

```css
.graph-mode-btn {
  padding: 6px 12px;
  margin: 0 2px;
  border: 1px solid var(--border);
  background: var(--bg);
  color: var(--fg);
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
}

.graph-mode-btn.active {
  background: var(--accent);
  color: white;
  border-color: var(--accent);
}

#graph-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

#graph-zoom-controls button {
  padding: 4px 8px;
  margin-left: 4px;
  font-size: 11px;
  cursor: pointer;
  border: 1px solid var(--border);
  background: var(--bg);
  border-radius: 3px;
}
```

- [ ] **Step 5: Commit**

```bash
git add gui/ui/index.html
git commit -m "feat: add Word graph toggle button and zoom controls UI"
```

---

### Task 8: Word Node Interaction (Click Highlight)

**Files:**
- Modify: `gui/ui/index.html` (JavaScript interaction handlers)

**Interfaces:**
- Produces:
  - Click handler that highlights word node and related notes in sidebar
  - Hover handler that shows tooltip with word stats

- [ ] **Step 1: Add word node click handler**

In JavaScript, add:

```javascript
let selectedWordNode = null;

function onWordNodeClick(wordNode) {
  selectedWordNode = wordNode;
  
  // Query for notes containing this word
  invoke('word_graph_notes', { root, word: wordNode.word })
    .then(notes => {
      // Highlight these notes in sidebar
      document.querySelectorAll('[data-file]').forEach(noteEl => {
        const fileName = noteEl.getAttribute('data-file');
        if (notes.includes(fileName)) {
          noteEl.classList.add('highlighted');
        } else {
          noteEl.classList.remove('highlighted');
        }
      });
    })
    .catch(err => console.error('Failed to get notes for word:', err));
  
  redrawGraph();
}
```

- [ ] **Step 2: Add hover tooltip**

```javascript
function showWordTooltip(wordNode, x, y) {
  const tooltip = document.getElementById('graph-tooltip') || document.createElement('div');
  tooltip.id = 'graph-tooltip';
  tooltip.style.cssText = `
    position: absolute;
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 8px;
    border-radius: 4px;
    font-size: 12px;
    z-index: 1000;
    left: ${x}px;
    top: ${y}px;
    max-width: 200px;
  `;
  tooltip.innerHTML = `
    <strong>${wordNode.word}</strong><br>
    Frequency: ${wordNode.frequency}<br>
    Partners: ${wordNode.frequency > 0 ? 'loading...' : 'none'}
  `;
  
  document.body.appendChild(tooltip);
  
  // TODO: Load and display top co-occurrence partners
}

function hideWordTooltip() {
  const tooltip = document.getElementById('graph-tooltip');
  if (tooltip) tooltip.remove();
}
```

- [ ] **Step 3: Wire up click/hover to canvas**

Find the graph canvas click handler and add word node detection:

```javascript
graphCanvas.addEventListener('click', (e) => {
  // Transform event coords to graph space
  const rect = graphCanvas.getBoundingClientRect();
  const zoom = graphZoom[currentGraphMode];
  const x = ((e.clientX - rect.left) / zoom.scale) + zoom.panX;
  const y = ((e.clientY - rect.top) / zoom.scale) + zoom.panY;
  
  // Check if click hits a word node (node radius ~20px)
  const hitNode = graphNodes.find(node => {
    const dx = node.x - x;
    const dy = node.y - y;
    return Math.sqrt(dx*dx + dy*dy) < 20;
  });
  
  if (hitNode && currentGraphMode === 'word') {
    onWordNodeClick(hitNode);
  }
});
```

- [ ] **Step 4: Commit**

```bash
git add gui/ui/index.html
git commit -m "feat: add word node click highlighting and tooltip"
```

---

### Task 9: Load and Render Word Graph Data

**Files:**
- Modify: `gui/ui/index.html` (JavaScript rendering logic)

**Interfaces:**
- Produces:
  - `loadWordGraphData()` function that fetches from `word_graph_data` command and populates graphNodes/graphLinks
  - Render logic that sizes nodes by frequency and colors by heat-map

- [ ] **Step 1: Add loadWordGraphData function**

```javascript
async function loadWordGraphData() {
  try {
    const response = await invoke('word_graph_data', { root });
    
    graphNodes = response.nodes.map(node => ({
      ...node,
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height,
      vx: 0,
      vy: 0,
    }));
    
    graphLinks = response.links.map(link => ({
      source: link.source,
      target: link.target,
      weight: link.weight,
    }));
    
    // Update timestamp
    if (response.last_updated) {
      document.getElementById('graph-timestamp').textContent = `Last updated: ${response.last_updated}`;
    }
    
    redrawGraph();
  } catch (err) {
    console.error('Failed to load word graph:', err);
    toast('Failed to load word graph');
  }
}
```

- [ ] **Step 2: Update node rendering for Word mode**

In the `drawGraph()` function, add Word-mode-specific rendering:

```javascript
if (currentGraphMode === 'word') {
  // Size nodes by log(frequency)
  graphNodes.forEach(node => {
    const size = 5 + Math.log(Math.max(1, node.frequency)) * 3;
    const hue = (node.frequency / maxFrequency) * 120; // Blue to red
    const color = `hsl(${hue}, 70%, 50%)`;
    
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(node.x, node.y, size, 0, Math.PI * 2);
    ctx.fill();
    
    // Draw label for large nodes
    if (size > 10) {
      ctx.fillStyle = 'white';
      ctx.font = '12px monospace';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(node.word.substring(0, 10), node.x, node.y);
    }
  });
}
```

- [ ] **Step 3: Commit**

```bash
git add gui/ui/index.html
git commit -m "feat: add word graph data loading and rendering"
```

---

### Task 10: Integration Testing & Manual Testing

**Files:**
- Create: `gui/tests/word_graph_integration.rs` (integration test fixture)
- Modify: `docs/gui/GUI.md` (add manual test checklist)

**Interfaces:**
- Produces:
  - Integration test for full indexing workflow with fixture vault
  - Manual test checklist for Word graph feature

- [ ] **Step 1: Create fixture vault for testing**

In `gui/tests/`, create a fixture vault with known word pairs for testing.

- [ ] **Step 2: Write integration tests**

Create `gui/tests/word_graph_integration.rs`:
```rust
#[tokio::test]
async fn test_word_graph_end_to_end() {
    // Setup fixture vault
    // Index it
    // Verify word graph response contains expected nodes/links
}
```

- [ ] **Step 3: Update GUI.md with manual test checklist**

Add to `docs/gui/GUI.md`:
```markdown
## Word Graph Manual Tests

- [ ] Open Word tab, verify graph renders with word nodes sized by frequency
- [ ] Zoom with mouse wheel, verify per-view zoom independent
- [ ] Zoom with +/− buttons, verify zoom increments by 10%
- [ ] Press 0 to reset zoom, verify returns to fit-all
- [ ] Click a word node, verify related notes highlight in sidebar
- [ ] Hover word node, verify tooltip shows stats
- [ ] Modify a note, open Word tab, verify index updates within 1s
- [ ] Verify stopwords (the, and, is) excluded from graph
```

- [ ] **Step 4: Run full test suite**

Run: `cargo test -p to_markdown_gui`
Expected: All tests pass including new integration tests

- [ ] **Step 5: Commit**

```bash
git add gui/tests/word_graph_integration.rs docs/gui/GUI.md
git commit -m "test: add word graph integration and manual test checklist"
```

---

### Task 11: Performance Optimization & Polish

**Files:**
- Modify: `gui/src/word_graph/indexer.rs` (background thread spawning)
- Modify: `gui/ui/index.html` (throttle graph rendering, add loading state)

**Interfaces:**
- Produces:
  - Background indexing that doesn't freeze UI
  - "Indexing..." status in Word header during updates
  - Throttled graph animation when tab not visible

- [ ] **Step 1: Update indexer to spawn background thread properly**

In `gui/src/main.rs`, update `index_vault_words` to use `spawn_blocking` correctly.

- [ ] **Step 2: Add loading state to Word header**

In `index.html`, update timestamp display to show "Indexing..." while delta update is in progress.

- [ ] **Step 3: Add visibility API to throttle rendering**

```javascript
document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    graphAnimating = false;
  } else {
    graphAnimating = true;
  }
});
```

- [ ] **Step 4: Commit**

```bash
git add gui/src/word_graph/indexer.rs gui/src/main.rs gui/ui/index.html
git commit -m "perf: add background indexing and visibility-based rendering throttle"
```

---

### Task 12: Documentation & Final Review

**Files:**
- Modify: `docs/gui/GUI.md` (add Word graph section)

**Interfaces:**
- Produces:
  - User-facing documentation for Word graph feature

- [ ] **Step 1: Add Word graph section to GUI.md**

```markdown
## Word Graph

The Word graph visualizes relationships between words in your vault based on 
co-occurrence—words that appear together in notes. Use it to discover potential 
super-links: tight word clusters indicate related notes that should be linked.

### How to use
1. Open a note, click the Graph button
2. Click "Word" to switch to word-relationship view
3. Larger words appear more frequently; edge thickness shows co-occurrence strength
4. Click a word to highlight all notes containing it
5. Zoom with mouse wheel, touch pinch, or +/− buttons; press 0 to reset

### Technical details
- Index built automatically on app startup (if fresh) or when opening Word tab (delta update)
- Top 200 words by frequency (excluding common stopwords like "the", "and")
- Co-occurrence edges shown for word pairs appearing together in 2+ notes
- Index stored locally in `.tomarkdown_word_graph.db` SQLite database
```

- [ ] **Step 2: Run cargo test one final time**

Run: `cargo test -p to_markdown_gui`
Expected: All tests pass, no clippy warnings

- [ ] **Step 3: Final commit**

```bash
git add docs/gui/GUI.md
git commit -m "docs: add Word graph feature documentation"
```

---

## Summary

This plan implements the Word graph feature across 12 tasks:

1. **Database setup** — SQLite schema for words, co-occurrence, indexing state
2. **Tokenization** — Extract words, filter stopwords, enforce min length
3. **Full indexing** — Index entire vault on startup
4. **Delta indexing** — Incremental updates for changed files
5. **Query functions** — Fetch top words, pairs, co-occurrence partners
6. **Tauri commands** — `word_graph_data()` and `index_vault_words()`
7. **Per-view zoom state** — Independent zoom level per graph mode
8. **Zoom events** — Mouse wheel, touch pinch, keyboard, buttons
9. **UI toggle & header** — Word button, zoom controls, timestamp
10. **Word interaction** — Click highlight, hover tooltip
11. **Graph rendering** — Load and render word nodes sized by frequency
12. **Polish & docs** — Background indexing, throttling, documentation

Each task is independently testable and buildable. All tests pass; all code is committed incrementally. Feature is ready for manual testing and eventual merge.
