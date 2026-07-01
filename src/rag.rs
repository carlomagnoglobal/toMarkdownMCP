//! AI/RAG toolkit: turn Markdown (or any converted document) into
//! machine-consumable retrieval units — chunks, outlines, word statistics,
//! and content search. Shared tokenizer lives here and is reused across tools
//! (and by the `knowledge` module).

use std::collections::HashMap;

/// A common English stopword list used for keyword/statistics filtering.
pub const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "if", "then", "else", "when", "at", "by", "for", "with",
    "about", "against", "between", "into", "through", "during", "before", "after", "above", "below",
    "to", "from", "up", "down", "in", "out", "on", "off", "over", "under", "again", "further",
    "of", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do", "does",
    "did", "this", "that", "these", "those", "i", "you", "he", "she", "it", "we", "they", "them",
    "his", "her", "its", "our", "their", "as", "so", "than", "too", "very", "can", "will", "just",
    "not", "no", "nor", "only", "own", "same", "such", "s", "t", "don", "now", "there", "here",
];

fn is_stopword(word: &str) -> bool {
    STOPWORDS.contains(&word)
}

/// Tokenize text into lowercase word tokens, stripping Markdown punctuation.
/// Reused by search ranking, statistics, and TF-IDF keyword extraction.
pub fn tokenize_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .filter_map(|w| {
            let w = w.trim_matches(|c: char| c == '\'' || c == '-').to_lowercase();
            if w.is_empty() {
                None
            } else {
                Some(w)
            }
        })
        .collect()
}

/// Strip common Markdown syntax so word counts reflect prose, not markup.
fn strip_markdown(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let mut l = line.trim_start_matches('#').to_string();
        l = l
            .replace("**", "")
            .replace('*', "")
            .replace('`', "")
            .replace('>', "")
            .replace('|', " ");
        out.push_str(&l);
        out.push('\n');
    }
    out
}

// ----------------------------------------------------------------------------
// Word / vocabulary statistics
// ----------------------------------------------------------------------------

pub struct TextStats {
    pub total_words: usize,
    pub distinct_words: usize,
    pub char_count: usize,
    pub sentence_count: usize,
    pub paragraph_count: usize,
    /// (word, count) sorted by count desc.
    pub frequencies: Vec<(String, usize)>,
}

/// Compute word/vocabulary statistics for a piece of text.
pub fn text_statistics(text: &str, exclude_stopwords: bool, min_length: usize) -> TextStats {
    let cleaned = strip_markdown(text);
    let tokens = tokenize_words(&cleaned);

    let total_words = tokens.len();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for tok in &tokens {
        if tok.len() < min_length {
            continue;
        }
        if exclude_stopwords && is_stopword(tok) {
            continue;
        }
        *counts.entry(tok.clone()).or_insert(0) += 1;
    }

    let distinct_words = counts.len();
    let mut frequencies: Vec<(String, usize)> = counts.into_iter().collect();
    frequencies.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let char_count = text.chars().count();
    let sentence_count = text
        .split(|c| c == '.' || c == '!' || c == '?')
        .filter(|s| !s.trim().is_empty())
        .count()
        .max(1);
    let paragraph_count = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .count()
        .max(1);

    TextStats {
        total_words,
        distinct_words,
        char_count,
        sentence_count,
        paragraph_count,
        frequencies,
    }
}

// ----------------------------------------------------------------------------
// Chunking
// ----------------------------------------------------------------------------

pub struct Chunk {
    pub text: String,
    pub heading_path: Vec<String>,
    pub token_estimate: usize,
}

/// Approximate token count (roughly words * 1.3, which tracks common
/// subword tokenizers without pulling in a model dependency).
pub fn estimate_tokens(text: &str) -> usize {
    let words = text.split_whitespace().count();
    ((words as f64) * 1.3).ceil() as usize
}

/// Split Markdown into heading-aware, token-bounded chunks. Sections are split
/// on headings first; oversized sections are further split by token budget with
/// a configurable overlap (in words).
pub fn chunk_markdown(md: &str, max_tokens: usize, overlap: usize) -> Vec<Chunk> {
    let max_tokens = max_tokens.max(32);
    let mut chunks = Vec::new();
    let mut heading_stack: Vec<(usize, String)> = Vec::new();
    let mut current = String::new();

    let current_path = |stack: &[(usize, String)]| -> Vec<String> {
        stack.iter().map(|(_, t)| t.clone()).collect()
    };

    let flush = |current: &mut String, path: Vec<String>, chunks: &mut Vec<Chunk>| {
        if current.trim().is_empty() {
            return;
        }
        // Sub-split if over the token budget.
        if estimate_tokens(current) > max_tokens {
            for piece in split_by_tokens(current, max_tokens, overlap) {
                let tok = estimate_tokens(&piece);
                chunks.push(Chunk {
                    text: piece,
                    heading_path: path.clone(),
                    token_estimate: tok,
                });
            }
        } else {
            let tok = estimate_tokens(current);
            chunks.push(Chunk {
                text: current.trim().to_string(),
                heading_path: path,
                token_estimate: tok,
            });
        }
        current.clear();
    };

    for line in md.lines() {
        if let Some(level) = heading_prefix_level(line) {
            // Close the current section before starting a new heading.
            flush(&mut current, current_path(&heading_stack), &mut chunks);
            let title = line.trim_start_matches('#').trim().to_string();
            while heading_stack.last().map(|(l, _)| *l >= level).unwrap_or(false) {
                heading_stack.pop();
            }
            heading_stack.push((level, title));
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    flush(&mut current, current_path(&heading_stack), &mut chunks);
    chunks
}

/// Return the heading level (1-6) if the line is an ATX Markdown heading.
fn heading_prefix_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.len() - trimmed.trim_start_matches('#').len();
    if (1..=6).contains(&level) && trimmed.as_bytes().get(level) == Some(&b' ') {
        Some(level)
    } else {
        None
    }
}

/// Split a block of text into token-bounded pieces with word overlap.
fn split_by_tokens(text: &str, max_tokens: usize, overlap: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![];
    }
    // Convert the token budget to an approximate word budget.
    let words_per_chunk = ((max_tokens as f64) / 1.3).floor().max(1.0) as usize;
    let step = words_per_chunk.saturating_sub(overlap).max(1);

    let mut pieces = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let end = (start + words_per_chunk).min(words.len());
        pieces.push(words[start..end].join(" "));
        if end == words.len() {
            break;
        }
        start += step;
    }
    pieces
}

// ----------------------------------------------------------------------------
// Outline
// ----------------------------------------------------------------------------

pub struct OutlineNode {
    pub level: usize,
    pub title: String,
    pub anchor: String,
    pub children: Vec<OutlineNode>,
}

/// Build a nested heading outline from Markdown.
pub fn build_outline(md: &str) -> Vec<OutlineNode> {
    let mut roots: Vec<OutlineNode> = Vec::new();
    for line in md.lines() {
        if let Some(level) = heading_prefix_level(line) {
            let title = line.trim_start_matches('#').trim().to_string();
            let node = OutlineNode {
                level,
                anchor: slugify(&title),
                title,
                children: Vec::new(),
            };
            insert_outline(&mut roots, node);
        }
    }
    roots
}

fn insert_outline(nodes: &mut Vec<OutlineNode>, node: OutlineNode) {
    match nodes.last_mut() {
        Some(last) if last.level < node.level => insert_outline(&mut last.children, node),
        _ => nodes.push(node),
    }
}

/// GitHub-style heading anchor slug.
pub fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c)
            } else if c == ' ' || c == '-' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_words() {
        let toks = tokenize_words("Hello, world! Rust-lang is great.");
        assert_eq!(toks, vec!["hello", "world", "rust-lang", "is", "great"]);
    }

    #[test]
    fn test_text_statistics_counts() {
        let stats = text_statistics("the cat sat on the mat. the cat ran.", false, 1);
        assert_eq!(stats.total_words, 9);
        // distinct: the, cat, sat, on, mat, ran = 6
        assert_eq!(stats.distinct_words, 6);
        assert_eq!(stats.frequencies[0].0, "the");
        assert_eq!(stats.frequencies[0].1, 3);
        assert_eq!(stats.sentence_count, 2);
    }

    #[test]
    fn test_text_statistics_stopwords() {
        let stats = text_statistics("the cat and the dog", true, 1);
        // "the" and "and" removed from distinct set
        assert!(stats.frequencies.iter().all(|(w, _)| w != "the" && w != "and"));
        assert_eq!(stats.total_words, 5);
    }

    #[test]
    fn test_chunk_markdown_respects_headings() {
        let md = "# Title\n\nIntro text.\n\n## Section A\n\nContent A.\n\n## Section B\n\nContent B.";
        let chunks = chunk_markdown(md, 512, 0);
        assert!(chunks.len() >= 3);
        let a = chunks.iter().find(|c| c.text.contains("Content A")).unwrap();
        assert_eq!(a.heading_path, vec!["Title", "Section A"]);
    }

    #[test]
    fn test_chunk_markdown_splits_large_section() {
        let body = "word ".repeat(400);
        let md = format!("# Big\n\n{}", body);
        let chunks = chunk_markdown(&md, 100, 10);
        assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());
        assert!(chunks.iter().all(|c| c.token_estimate <= 120));
    }

    #[test]
    fn test_build_outline_nesting() {
        let md = "# A\n## B\n## C\n### D\n# E";
        let outline = build_outline(md);
        assert_eq!(outline.len(), 2); // A, E
        assert_eq!(outline[0].title, "A");
        assert_eq!(outline[0].children.len(), 2); // B, C
        assert_eq!(outline[0].children[1].children[0].title, "D");
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("Section 1.2"), "section-12");
    }
}
