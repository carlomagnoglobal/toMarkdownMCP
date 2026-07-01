//! "Second brain" knowledge tools: turn a Markdown vault into an AI-ready
//! knowledge layer — tags, TF-IDF keywords, related-note similarity, extractive
//! summaries, Q/A pairs, and lightweight entity extraction. All extractive and
//! deterministic (no LLM dependency); file IO lives in the handlers in main.rs.

use std::collections::{HashMap, HashSet};

use crate::rag::{tokenize_words, STOPWORDS};

fn is_stopword(w: &str) -> bool {
    STOPWORDS.contains(&w)
}

// ----------------------------------------------------------------------------
// Tags
// ----------------------------------------------------------------------------

/// Extract `#tags` (inline hashtags) and YAML frontmatter `tags:` values.
pub fn extract_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();

    // Frontmatter tags (simple `tags: [a, b]` or `tags: a, b`).
    if let Some(fm) = frontmatter_block(text) {
        for line in fm.lines() {
            if let Some(rest) = line.trim().strip_prefix("tags:") {
                for t in rest
                    .trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(&[',', ' '][..])
                {
                    let t = t.trim().trim_matches(['"', '\'']).trim_start_matches('#');
                    if !t.is_empty() {
                        tags.push(t.to_string());
                    }
                }
            }
        }
    }

    // Inline hashtags: #tag (not Markdown headings, which are `# ` with a space).
    for token in text.split_whitespace() {
        if let Some(tag) = token.strip_prefix('#') {
            let tag: String = tag
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '/')
                .collect();
            // Require at least one letter to avoid matching "#1" numeric refs.
            if tag.chars().any(|c| c.is_alphabetic()) {
                tags.push(tag);
            }
        }
    }
    tags
}

/// Return the YAML frontmatter block (between leading `---` fences), if present.
fn frontmatter_block(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

// ----------------------------------------------------------------------------
// Term frequencies / keywords / similarity
// ----------------------------------------------------------------------------

/// Term-frequency map for a document, excluding stopwords and very short words.
pub fn term_frequencies(text: &str) -> HashMap<String, usize> {
    let mut tf = HashMap::new();
    for tok in tokenize_words(text) {
        if tok.len() < 3 || is_stopword(&tok) {
            continue;
        }
        *tf.entry(tok).or_insert(0) += 1;
    }
    tf
}

/// Compute TF-IDF scores for a document given document frequencies over a
/// corpus. `num_docs` is the total corpus size; `doc_freq[term]` is how many
/// documents contain the term. With an empty corpus this degrades to TF.
pub fn tf_idf(
    tf: &HashMap<String, usize>,
    doc_freq: &HashMap<String, usize>,
    num_docs: usize,
) -> Vec<(String, f64)> {
    let n = num_docs.max(1) as f64;
    let mut scores: Vec<(String, f64)> = tf
        .iter()
        .map(|(term, &count)| {
            let df = *doc_freq.get(term).unwrap_or(&0) as f64;
            let idf = ((n + 1.0) / (df + 1.0)).ln() + 1.0;
            (term.clone(), count as f64 * idf)
        })
        .collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

/// Cosine similarity between two term-frequency vectors.
pub fn cosine_similarity(a: &HashMap<String, usize>, b: &HashMap<String, usize>) -> f64 {
    let mut dot = 0.0;
    for (term, &ca) in a {
        if let Some(&cb) = b.get(term) {
            dot += (ca * cb) as f64;
        }
    }
    let na: f64 = a.values().map(|&c| (c * c) as f64).sum::<f64>().sqrt();
    let nb: f64 = b.values().map(|&c| (c * c) as f64).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Terms shared between two documents (intersection of their vocabularies).
pub fn shared_terms(a: &HashMap<String, usize>, b: &HashMap<String, usize>) -> Vec<String> {
    let bset: HashSet<&String> = b.keys().collect();
    let mut shared: Vec<String> = a.keys().filter(|k| bset.contains(*k)).cloned().collect();
    shared.sort();
    shared
}

// ----------------------------------------------------------------------------
// Extractive summary
// ----------------------------------------------------------------------------

/// Rank sentences by keyword density (+ a small position bias) and return the
/// top `n` in original order.
pub fn summarize(text: &str, n: usize) -> Vec<String> {
    let sentences = split_sentences(text);
    if sentences.len() <= n {
        return sentences;
    }

    // Global term weights = document term frequencies.
    let tf = term_frequencies(text);

    let mut scored: Vec<(usize, f64, &String)> = sentences
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let toks = tokenize_words(s);
            let word_score: usize = toks
                .iter()
                .filter(|t| t.len() >= 3 && !is_stopword(t))
                .map(|t| *tf.get(t).unwrap_or(&0))
                .sum();
            let density = if toks.is_empty() {
                0.0
            } else {
                word_score as f64 / toks.len() as f64
            };
            // Small bias toward earlier sentences.
            let position_bonus = 1.0 / (1.0 + i as f64 * 0.1);
            (i, density * position_bonus, s)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut chosen: Vec<usize> = scored.iter().take(n).map(|(i, _, _)| *i).collect();
    chosen.sort_unstable();
    chosen.into_iter().map(|i| sentences[i].clone()).collect()
}

/// Split text into sentences on `.!?` boundaries, ignoring Markdown structure.
fn split_sentences(text: &str) -> Vec<String> {
    let flat = text.replace('\n', " ");
    let mut sentences = Vec::new();
    let mut current = String::new();
    for c in flat.chars() {
        current.push(c);
        if matches!(c, '.' | '!' | '?') {
            let trimmed = current.trim();
            if trimmed.len() > 1 {
                sentences.push(trimmed.to_string());
            }
            current.clear();
        }
    }
    let trimmed = current.trim();
    if trimmed.len() > 1 {
        sentences.push(trimmed.to_string());
    }
    sentences
}

// ----------------------------------------------------------------------------
// Q/A pairs
// ----------------------------------------------------------------------------

/// Extract question/answer pairs from `Q:/A:` lines and `?`-terminated headings
/// (answer = following text until the next heading).
pub fn extract_qa_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut pending_q: Option<String> = None;

    while i < lines.len() {
        let line = lines[i].trim();

        // Q:/A: style.
        if let Some(q) = line.strip_prefix("Q:").or_else(|| line.strip_prefix("**Q:**")) {
            // Look for the next A: line.
            let mut answer = String::new();
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j].trim();
                if let Some(a) = l.strip_prefix("A:").or_else(|| l.strip_prefix("**A:**")) {
                    answer = a.trim().to_string();
                    break;
                }
                j += 1;
            }
            pairs.push((q.trim().to_string(), answer));
            i = j + 1;
            continue;
        }

        // Heading ending in '?' -> question; collect body until next heading.
        if line.starts_with('#') && line.trim_end().ends_with('?') {
            if let Some(q) = pending_q.take() {
                pairs.push((q, String::new()));
            }
            pending_q = Some(line.trim_start_matches('#').trim().to_string());
            let mut body = String::new();
            let mut j = i + 1;
            while j < lines.len() && !lines[j].trim().starts_with('#') {
                if !lines[j].trim().is_empty() {
                    body.push_str(lines[j].trim());
                    body.push(' ');
                }
                j += 1;
            }
            if let Some(q) = pending_q.take() {
                pairs.push((q, body.trim().to_string()));
            }
            i = j;
            continue;
        }

        i += 1;
    }
    pairs
}

// ----------------------------------------------------------------------------
// Entities (lightweight)
// ----------------------------------------------------------------------------

#[derive(Clone)]
pub struct Entity {
    pub value: String,
    pub kind: &'static str,
}

/// Extract lightweight entities: URLs, emails, dates, and capitalized phrases.
pub fn extract_entities(text: &str) -> Vec<Entity> {
    let mut seen: HashSet<(String, &'static str)> = HashSet::new();
    let mut entities: Vec<Entity> = Vec::new();

    let mut push = |value: String, kind: &'static str, entities: &mut Vec<Entity>| {
        if value.len() < 2 {
            return;
        }
        if seen.insert((value.clone(), kind)) {
            entities.push(Entity { value, kind });
        }
    };

    for token in text.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != ':' && c != '/' && c != '.' && c != '-');
        if clean.starts_with("http://") || clean.starts_with("https://") {
            push(clean.to_string(), "url", &mut entities);
        } else if clean.contains('@') && clean.contains('.') && !clean.contains("://") {
            push(clean.to_string(), "email", &mut entities);
        } else if is_date_like(clean.trim_end_matches('.')) {
            push(clean.trim_end_matches('.').to_string(), "date", &mut entities);
        }
    }

    // Capitalized multi-word phrases (naive proper-noun detection).
    for phrase in capitalized_phrases(text) {
        push(phrase, "name", &mut entities);
    }
    entities
}

fn is_date_like(s: &str) -> bool {
    // Matches YYYY-MM-DD or YYYY/MM/DD.
    let parts: Vec<&str> = s.split(&['-', '/'][..]).collect();
    parts.len() == 3
        && parts[0].len() == 4
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Find sequences of Capitalized words (length >= 2 words) not at sentence start.
fn capitalized_phrases(text: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    for sentence in text.replace('\n', " ").split(['.', '!', '?', ',', ';', ':']) {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        let mut current: Vec<&str> = Vec::new();
        for (idx, w) in words.iter().enumerate() {
            let is_cap = w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && w.chars().any(|c| c.is_alphabetic());
            // Skip the first word of a sentence (usually capitalized by grammar).
            if is_cap && idx > 0 {
                current.push(w);
            } else {
                if current.len() >= 2 {
                    phrases.push(current.join(" "));
                }
                current.clear();
            }
        }
        if current.len() >= 2 {
            phrases.push(current.join(" "));
        }
    }
    phrases
}

/// Count occurrences into a (value, count) list sorted by count desc.
pub fn count_and_rank(items: Vec<String>) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for it in items {
        *counts.entry(it).or_insert(0) += 1;
    }
    let mut v: Vec<(String, usize)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tags_inline_and_frontmatter() {
        let text = "---\ntags: [rust, mcp]\n---\n\nSome #markdown and #ai/ml notes. Ref #1 ignored.";
        let tags = extract_tags(text);
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"mcp".to_string()));
        assert!(tags.contains(&"markdown".to_string()));
        assert!(tags.contains(&"ai/ml".to_string()));
        assert!(!tags.contains(&"1".to_string()));
    }

    #[test]
    fn test_cosine_similarity() {
        let a = term_frequencies("rust systems programming rust");
        let b = term_frequencies("rust programming language");
        let c = term_frequencies("cooking recipes food");
        assert!(cosine_similarity(&a, &b) > cosine_similarity(&a, &c));
    }

    #[test]
    fn test_tf_idf_ranks_rare_terms() {
        let tf = term_frequencies("apple apple banana");
        let mut df = HashMap::new();
        df.insert("apple".to_string(), 10);
        df.insert("banana".to_string(), 1);
        let scores = tf_idf(&tf, &df, 10);
        // banana is rarer, should outrank apple despite lower TF.
        assert_eq!(scores[0].0, "banana");
    }

    #[test]
    fn test_summarize_returns_n() {
        let text = "The system is fast. It handles many files. Speed matters a lot. Errors are rare. Users are happy.";
        let summary = summarize(text, 2);
        assert_eq!(summary.len(), 2);
    }

    #[test]
    fn test_extract_qa_pairs() {
        let text = "Q: What is Rust?\nA: A systems language.\n\n## How does it work?\nIt compiles to native code.";
        let pairs = extract_qa_pairs(text);
        assert!(pairs.iter().any(|(q, a)| q == "What is Rust?" && a == "A systems language."));
        assert!(pairs.iter().any(|(q, _)| q == "How does it work?"));
    }

    #[test]
    fn test_extract_entities() {
        let text = "Contact John Smith at john@example.com or visit https://example.com on 2026-06-30.";
        let ents = extract_entities(text);
        assert!(ents.iter().any(|e| e.kind == "email" && e.value == "john@example.com"));
        assert!(ents.iter().any(|e| e.kind == "url"));
        assert!(ents.iter().any(|e| e.kind == "date" && e.value == "2026-06-30"));
        assert!(ents.iter().any(|e| e.kind == "name" && e.value == "John Smith"));
    }
}
