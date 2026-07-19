use crate::word_graph::db::WordGraphDb;
use crate::word_graph::tokenizer::{tokenize, get_word_pairs};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

pub fn index_vault_full(db: &WordGraphDb, vault_path: &Path) -> Result<()> {
    // Find all markdown files
    let md_files = find_markdown_files(vault_path)?;

    // Clear existing index
    db.conn_mut().execute("DELETE FROM co_occurrence", [])?;
    db.conn_mut().execute("DELETE FROM words", [])?;

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
    let tx = db.conn_mut().transaction()?;

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

    let tx = db.conn_mut().transaction()?;

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
                [(existing + delta).to_string(), word],
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
