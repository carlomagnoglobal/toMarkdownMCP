//! Local RAG retrieval: rank chunks against a query and assemble a context
//! block under a token budget. Pure ranking helpers; file IO lives in the
//! handlers in main.rs. Reuses the shared tokenizer and chunker from `rag`.

use std::collections::HashSet;

use crate::rag::{estimate_tokens, tokenize_words, Chunk};

/// A scored chunk with provenance, ready to assemble into a context window.
pub struct ScoredChunk {
    pub source: String,
    pub heading_path: Vec<String>,
    pub text: String,
    pub score: f64,
    pub token_estimate: usize,
}

/// Score a single chunk against a set of query terms. Score = sum over query
/// terms of (term frequency in chunk), normalized by chunk length, so short
/// on-topic chunks aren't buried by long ones.
pub fn score_chunk(chunk_text: &str, query_terms: &HashSet<String>) -> f64 {
    if query_terms.is_empty() {
        return 0.0;
    }
    let toks = tokenize_words(chunk_text);
    if toks.is_empty() {
        return 0.0;
    }
    let hits = toks.iter().filter(|t| query_terms.contains(*t)).count();
    // Reward coverage of distinct query terms too.
    let distinct_hits = query_terms
        .iter()
        .filter(|q| toks.iter().any(|t| &t == q))
        .count();
    (hits as f64 / toks.len() as f64) * 100.0 + distinct_hits as f64
}

/// Rank chunks (each paired with its source path) against a query and return
/// them sorted by descending score, dropping zero-score chunks.
pub fn rank_chunks(query: &str, chunks: Vec<(String, Chunk)>) -> Vec<ScoredChunk> {
    let query_terms: HashSet<String> = tokenize_words(query).into_iter().collect();

    let mut scored: Vec<ScoredChunk> = chunks
        .into_iter()
        .map(|(source, c)| {
            let score = score_chunk(&c.text, &query_terms);
            ScoredChunk {
                source,
                heading_path: c.heading_path,
                text: c.text,
                score,
                token_estimate: c.token_estimate,
            }
        })
        .filter(|c| c.score > 0.0)
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

/// Select the top chunks that fit within `max_tokens`, up to `top_k` chunks.
/// Assumes `ranked` is already sorted by descending score.
pub fn select_within_budget(
    ranked: Vec<ScoredChunk>,
    max_tokens: usize,
    top_k: usize,
) -> Vec<ScoredChunk> {
    let mut selected = Vec::new();
    let mut used = 0usize;
    for chunk in ranked {
        if selected.len() >= top_k {
            break;
        }
        let cost = chunk.token_estimate.max(estimate_tokens(&chunk.text));
        if used + cost > max_tokens && !selected.is_empty() {
            continue; // skip oversized chunk but keep looking for smaller ones
        }
        used += cost;
        selected.push(chunk);
        if used >= max_tokens {
            break;
        }
    }
    selected
}

/// Assemble selected chunks into a single context block with lightweight
/// provenance headers, suitable to feed an LLM prompt.
pub fn assemble_context(chunks: &[ScoredChunk]) -> String {
    let mut out = String::new();
    for (i, c) in chunks.iter().enumerate() {
        let loc = if c.heading_path.is_empty() {
            c.source.clone()
        } else {
            format!("{} › {}", c.source, c.heading_path.join(" › "))
        };
        out.push_str(&format!("[{}] {}\n{}\n\n", i + 1, loc, c.text.trim()));
    }
    out.trim_end().to_string()
}

/// Context window sizes (in tokens) for known models, used by `count_tokens`.
pub fn model_context_windows() -> Vec<(&'static str, usize)> {
    vec![
        ("claude-opus-4-8", 200_000),
        ("claude-sonnet-5", 200_000),
        ("claude-haiku-4-5", 200_000),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::Chunk;

    fn chunk(source: &str, text: &str) -> (String, Chunk) {
        (
            source.to_string(),
            Chunk {
                text: text.to_string(),
                heading_path: vec![],
                token_estimate: estimate_tokens(text),
            },
        )
    }

    #[test]
    fn test_score_chunk_prefers_on_topic() {
        let q: HashSet<String> = tokenize_words("rust memory safety").into_iter().collect();
        let on = score_chunk("Rust guarantees memory safety without a garbage collector", &q);
        let off = score_chunk("The weather today is sunny and warm", &q);
        assert!(on > off);
        assert_eq!(off, 0.0);
    }

    #[test]
    fn test_rank_orders_by_score() {
        let chunks = vec![
            chunk("a.md", "cooking pasta and sauce"),
            chunk("b.md", "rust ownership and borrowing in rust"),
            chunk("c.md", "a note about rust briefly"),
        ];
        let ranked = rank_chunks("rust ownership", chunks);
        assert_eq!(ranked[0].source, "b.md");
        // The cooking chunk scores zero and is dropped.
        assert!(ranked.iter().all(|c| c.source != "a.md"));
    }

    #[test]
    fn test_select_within_budget() {
        let ranked = vec![
            ScoredChunk { source: "a".into(), heading_path: vec![], text: "word ".repeat(50), score: 5.0, token_estimate: 65 },
            ScoredChunk { source: "b".into(), heading_path: vec![], text: "word ".repeat(50), score: 4.0, token_estimate: 65 },
            ScoredChunk { source: "c".into(), heading_path: vec![], text: "word ".repeat(50), score: 3.0, token_estimate: 65 },
        ];
        let selected = select_within_budget(ranked, 100, 8);
        // Only the first fits under a 100-token budget.
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].source, "a");
    }

    #[test]
    fn test_assemble_context_has_citations() {
        let chunks = vec![ScoredChunk {
            source: "notes.md".into(),
            heading_path: vec!["Intro".into()],
            text: "hello world".into(),
            score: 1.0,
            token_estimate: 3,
        }];
        let ctx = assemble_context(&chunks);
        assert!(ctx.contains("[1] notes.md › Intro"));
        assert!(ctx.contains("hello world"));
    }
}
