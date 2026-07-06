use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::HashMap;

/// Complete metrics for a text: counts plus sorted frequency tables.
#[derive(Debug, Serialize)]
pub struct TextMetrics {
    pub words: usize,
    /// Every char: letters, digits, punctuation, whitespace.
    pub chars: usize,
    /// ASCII space characters only (tabs/newlines count in `chars`).
    pub spaces: usize,
    pub tokens: usize,
    /// False when the token count is an estimate rather than a real tokenizer.
    pub exact: bool,
    /// Human-readable description of the tokenization method.
    pub method: String,
    /// (word, count) sorted by count desc, ties alphabetical.
    pub word_freq: Vec<(String, usize)>,
    /// (character, count) sorted by count desc, ties alphabetical.
    /// Whitespace/control chars shown escaped (␣, \n, \t).
    pub char_freq: Vec<(String, usize)>,
    /// (token text, count) sorted by count desc, ties alphabetical.
    /// Empty for pure estimates (no real tokens to tabulate).
    pub token_freq: Vec<(String, usize)>,
}

/// Which tokenizer to use. Modular so new providers slot in.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenizerSpec {
    /// OpenAI tiktoken (exact, offline): model decides the encoding.
    OpenAi { model: String },
    /// Anthropic/Claude: no public local tokenizer — cl100k proxy, estimate.
    Anthropic,
    /// HuggingFace tokenizer.json (Meta Llama, Qwen, DeepSeek, ...). Exact
    /// when a file is supplied; falls back to a heuristic estimate otherwise.
    HuggingFace { provider: String, tokenizer_file: Option<String> },
    /// Grok: no public tokenizer — heuristic estimate.
    Grok,
    /// Plain chars/4 heuristic.
    Heuristic,
}

impl TokenizerSpec {
    /// Build a spec from tool parameters.
    pub fn from_params(provider: &str, model: Option<&str>, tokenizer_file: Option<&str>) -> Result<Self> {
        match provider.to_lowercase().as_str() {
            "openai" | "gpt" => Ok(TokenizerSpec::OpenAi {
                model: model.unwrap_or("gpt-4o").to_string(),
            }),
            "anthropic" | "claude" => Ok(TokenizerSpec::Anthropic),
            "meta" | "llama" | "qwen" | "deepseek" | "huggingface" | "hf" => {
                Ok(TokenizerSpec::HuggingFace {
                    provider: provider.to_lowercase(),
                    tokenizer_file: tokenizer_file.map(|s| s.to_string()),
                })
            }
            "grok" | "xai" => Ok(TokenizerSpec::Grok),
            "heuristic" | "estimate" => Ok(TokenizerSpec::Heuristic),
            other => Err(anyhow!(
                "Unknown provider '{}' (use openai|anthropic|meta|llama|qwen|deepseek|grok|heuristic)",
                other
            )),
        }
    }
}

/// chars/4 — the same heuristic the `count_tokens` tool uses.
fn heuristic_tokens(text: &str) -> usize {
    crate::rag::estimate_tokens(text)
}

fn tiktoken_bpe(model: &str) -> Result<tiktoken_rs::CoreBPE> {
    // o200k for the 4o/o-series generation, cl100k for gpt-4/3.5 era.
    let m = model.to_lowercase();
    if m.contains("4o") || m.starts_with("o1") || m.starts_with("o3") || m.starts_with("gpt-5") {
        tiktoken_rs::o200k_base().map_err(|e| anyhow!("tiktoken init failed: {}", e))
    } else {
        tiktoken_rs::cl100k_base().map_err(|e| anyhow!("tiktoken init failed: {}", e))
    }
}

fn encoding_name(model: &str) -> &'static str {
    let m = model.to_lowercase();
    if m.contains("4o") || m.starts_with("o1") || m.starts_with("o3") || m.starts_with("gpt-5") {
        "o200k_base"
    } else {
        "cl100k_base"
    }
}

/// Make BPE piece text readable in a table: mark leading spaces, escape
/// control characters, and the Ġ/▁ prefixes HF tokenizers use for spaces.
fn display_token(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        match c {
            ' ' => out.push('␣'),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            'Ġ' | '▁' => out.push('␣'),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    if out.is_empty() {
        "(empty)".to_string()
    } else {
        out
    }
}

fn sort_freq(map: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

/// Tokenize + tabulate. Returns (count, exact, method, token_freq).
fn tokenize(text: &str, spec: &TokenizerSpec) -> Result<(usize, bool, String, Vec<(String, usize)>)> {
    match spec {
        TokenizerSpec::OpenAi { model } => {
            let bpe = tiktoken_bpe(model)?;
            let ids = bpe.encode_with_special_tokens(text);
            let mut freq: HashMap<String, usize> = HashMap::new();
            for &id in &ids {
                let piece = bpe
                    .decode(vec![id])
                    .unwrap_or_else(|_| format!("<{}>", id));
                *freq.entry(display_token(&piece)).or_default() += 1;
            }
            Ok((
                ids.len(),
                true,
                format!("tiktoken {} for {} (exact)", encoding_name(model), model),
                sort_freq(freq),
            ))
        }
        TokenizerSpec::Anthropic => {
            // cl100k as a documented proxy: real Claude counts differ, so this
            // is labeled an estimate even though a real BPE runs underneath.
            let bpe = tiktoken_rs::cl100k_base().map_err(|e| anyhow!("tiktoken init failed: {}", e))?;
            let ids = bpe.encode_with_special_tokens(text);
            let mut freq: HashMap<String, usize> = HashMap::new();
            for &id in &ids {
                let piece = bpe
                    .decode(vec![id])
                    .unwrap_or_else(|_| format!("<{}>", id));
                *freq.entry(display_token(&piece)).or_default() += 1;
            }
            Ok((
                ids.len(),
                false,
                "cl100k_base proxy (ESTIMATE — Anthropic publishes no local tokenizer; \
                 use the count_tokens API endpoint for exact counts)"
                    .to_string(),
                sort_freq(freq),
            ))
        }
        TokenizerSpec::HuggingFace { provider, tokenizer_file } => match tokenizer_file {
            Some(path) => {
                let tok = tokenizers::Tokenizer::from_file(path)
                    .map_err(|e| anyhow!("Cannot load tokenizer.json '{}': {}", path, e))?;
                let enc = tok
                    .encode(text, false)
                    .map_err(|e| anyhow!("Tokenization failed: {}", e))?;
                let ids = enc.get_ids();
                let mut freq: HashMap<String, usize> = HashMap::new();
                for &id in ids {
                    let piece = tok.id_to_token(id).unwrap_or_else(|| format!("<{}>", id));
                    *freq.entry(display_token(&piece)).or_default() += 1;
                }
                Ok((
                    ids.len(),
                    true,
                    format!("HuggingFace tokenizer {} for {} (exact)", path, provider),
                    sort_freq(freq),
                ))
            }
            None => Ok((
                heuristic_tokens(text),
                false,
                format!(
                    "chars/4 heuristic (ESTIMATE — pass tokenizer_file with the {} model's \
                     tokenizer.json for exact counts)",
                    provider
                ),
                Vec::new(),
            )),
        },
        TokenizerSpec::Grok => Ok((
            heuristic_tokens(text),
            false,
            "chars/4 heuristic (ESTIMATE — xAI publishes no local tokenizer)".to_string(),
            Vec::new(),
        )),
        TokenizerSpec::Heuristic => Ok((
            heuristic_tokens(text),
            false,
            "chars/4 heuristic (ESTIMATE)".to_string(),
            Vec::new(),
        )),
    }
}

/// Analyze a text: word/char/space/token counts plus sorted frequency tables.
pub fn analyze_text(text: &str, spec: &TokenizerSpec) -> Result<TextMetrics> {
    let chars = text.chars().count();
    let spaces = text.chars().filter(|&c| c == ' ').count();

    // Words: whitespace-split, trimmed of surrounding punctuation, lowercased
    // for the frequency table (same normalization family as rag::text_statistics).
    let mut words = 0usize;
    let mut word_freq: HashMap<String, usize> = HashMap::new();
    for raw in text.split_whitespace() {
        let w: String = raw
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        words += 1;
        if !w.is_empty() {
            *word_freq.entry(w).or_default() += 1;
        }
    }

    // Character frequency (every char, escaped for display).
    let mut char_freq: HashMap<String, usize> = HashMap::new();
    for c in text.chars() {
        *char_freq.entry(display_token(&c.to_string())).or_default() += 1;
    }

    let (tokens, exact, method, token_freq) = tokenize(text, spec)?;

    Ok(TextMetrics {
        words,
        chars,
        spaces,
        tokens,
        exact,
        method,
        word_freq: sort_freq(word_freq),
        char_freq: sort_freq(char_freq),
        token_freq,
    })
}

/// Render metrics as readable Markdown (`top` limits table rows; 0 = all).
pub fn metrics_to_markdown(m: &TextMetrics, top: usize) -> String {
    let mut out = String::new();
    out.push_str("# Text Analysis\n\n");
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- **Words:** {}\n", m.words));
    out.push_str(&format!("- **Characters:** {} (letters, digits, punctuation, whitespace)\n", m.chars));
    out.push_str(&format!("- **Spaces:** {}\n", m.spaces));
    out.push_str(&format!(
        "- **Tokens:** {}{}\n",
        m.tokens,
        if m.exact { "" } else { " (estimated)" }
    ));
    out.push_str(&format!("- **Tokenization:** {}\n", m.method));
    if !m.exact {
        out.push_str("\n> ⚠️ Token count is an ESTIMATE, not an exact tokenizer result.\n");
    }

    let limit = |len: usize| if top == 0 { len } else { top.min(len) };

    out.push_str("\n## Word frequency\n\n");
    if m.word_freq.is_empty() {
        out.push_str("(no words)\n");
    }
    for (w, c) in m.word_freq.iter().take(limit(m.word_freq.len())) {
        out.push_str(&format!("{} = {}\n", w, c));
    }
    if top != 0 && m.word_freq.len() > top {
        out.push_str(&format!("… ({} more)\n", m.word_freq.len() - top));
    }

    out.push_str("\n## Character frequency\n\n");
    if m.char_freq.is_empty() {
        out.push_str("(no characters)\n");
    }
    for (ch, c) in m.char_freq.iter().take(limit(m.char_freq.len())) {
        out.push_str(&format!("{} = {}\n", ch, c));
    }
    if top != 0 && m.char_freq.len() > top {
        out.push_str(&format!("… ({} more)\n", m.char_freq.len() - top));
    }

    out.push_str("\n## Token frequency\n\n");
    if m.token_freq.is_empty() {
        out.push_str(if m.exact {
            "(no tokens)\n"
        } else {
            "(not available for estimated tokenization — no real tokens to tabulate)\n"
        });
    }
    for (t, c) in m.token_freq.iter().take(limit(m.token_freq.len())) {
        out.push_str(&format!("{} = {}\n", t, c));
    }
    if top != 0 && m.token_freq.len() > top {
        out.push_str(&format!("… ({} more)\n", m.token_freq.len() - top));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_all_zeros() {
        let m = analyze_text("", &TokenizerSpec::Heuristic).unwrap();
        assert_eq!((m.words, m.chars, m.spaces, m.tokens), (0, 0, 0, 0));
        assert!(m.word_freq.is_empty());
        assert!(m.token_freq.is_empty());
        assert!(!m.exact);
        let md = metrics_to_markdown(&m, 50);
        assert!(md.contains("**Words:** 0"));
    }

    #[test]
    fn short_text_counts_verifiable() {
        // "a bb a" -> 6 chars, 2 spaces, 3 words; freq a=2 bb=1
        let m = analyze_text("a bb a", &TokenizerSpec::Heuristic).unwrap();
        assert_eq!(m.words, 3);
        assert_eq!(m.chars, 6);
        assert_eq!(m.spaces, 2);
        assert_eq!(m.word_freq, vec![("a".to_string(), 2), ("bb".to_string(), 1)]);
    }

    #[test]
    fn chars_include_punctuation_and_newlines_spaces_ascii_only() {
        let m = analyze_text("Hi, there!\n\tok", &TokenizerSpec::Heuristic).unwrap();
        assert_eq!(m.chars, 14); // H i , space t h e r e ! \n \t o k
        assert_eq!(m.spaces, 1); // only the ASCII space; \n and \t excluded
        assert_eq!(m.words, 3);
        // punctuation stripped in frequency table
        assert!(m.word_freq.iter().any(|(w, c)| w == "hi" && *c == 1));
        assert!(m.word_freq.iter().any(|(w, _)| w == "there"));
    }

    #[test]
    fn char_frequency_counts_and_escapes() {
        // "aab a" -> a=3, space(␣)=1... wait: "aab a" = a,a,b,space,a
        let m = analyze_text("aab a", &TokenizerSpec::Heuristic).unwrap();
        assert_eq!(m.char_freq[0], ("a".to_string(), 3));
        assert!(m.char_freq.contains(&("b".to_string(), 1)));
        assert!(m.char_freq.contains(&("␣".to_string(), 1)));
        // Newlines escaped
        let m = analyze_text("x\n\ny", &TokenizerSpec::Heuristic).unwrap();
        assert!(m.char_freq.contains(&("\\n".to_string(), 2)));
        // Sorted desc, sum equals total chars
        let total: usize = m.char_freq.iter().map(|(_, c)| c).sum();
        assert_eq!(total, m.chars);
        let md = metrics_to_markdown(&m, 50);
        assert!(md.contains("## Character frequency"));
    }

    #[test]
    fn frequency_sorted_desc_then_alpha() {
        let m = analyze_text("b a b c a b", &TokenizerSpec::Heuristic).unwrap();
        assert_eq!(
            m.word_freq,
            vec![("b".into(), 3), ("a".into(), 2), ("c".into(), 1)]
        );
    }

    #[test]
    fn openai_exact_with_token_table() {
        let m = analyze_text("hello world hello", &TokenizerSpec::OpenAi { model: "gpt-4o".into() })
            .unwrap();
        assert!(m.exact);
        assert!(m.method.contains("o200k_base"));
        assert!(m.method.contains("exact"));
        assert!(m.tokens >= 3);
        assert!(!m.token_freq.is_empty());
        // Sorted desc
        for pair in m.token_freq.windows(2) {
            assert!(pair[0].1 >= pair[1].1);
        }
        // Leading-space pieces rendered visibly
        assert!(m.token_freq.iter().any(|(t, _)| t.starts_with('␣')));
    }

    #[test]
    fn gpt4_uses_cl100k() {
        let m = analyze_text("hi", &TokenizerSpec::OpenAi { model: "gpt-4".into() }).unwrap();
        assert!(m.method.contains("cl100k_base"));
    }

    #[test]
    fn anthropic_is_estimate_with_real_table() {
        let m = analyze_text("hello Claude", &TokenizerSpec::Anthropic).unwrap();
        assert!(!m.exact);
        assert!(m.method.contains("ESTIMATE"));
        assert!(m.method.contains("cl100k"));
        assert!(!m.token_freq.is_empty()); // proxy still yields a table
        let md = metrics_to_markdown(&m, 10);
        assert!(md.contains("⚠️"));
    }

    #[test]
    fn hf_without_file_estimates_and_warns() {
        let spec = TokenizerSpec::HuggingFace { provider: "qwen".into(), tokenizer_file: None };
        let m = analyze_text("some text here", &spec).unwrap();
        assert!(!m.exact);
        assert!(m.method.contains("tokenizer.json"));
        assert!(m.token_freq.is_empty());
    }

    #[test]
    fn hf_with_missing_file_errors_clearly() {
        let spec = TokenizerSpec::HuggingFace {
            provider: "meta".into(),
            tokenizer_file: Some("/nonexistent/tokenizer.json".into()),
        };
        let err = analyze_text("x", &spec).unwrap_err().to_string();
        assert!(err.contains("tokenizer.json"), "{}", err);
    }

    #[test]
    fn grok_estimates() {
        let m = analyze_text("hello grok", &TokenizerSpec::Grok).unwrap();
        assert!(!m.exact);
        assert!(m.method.contains("xAI"));
    }

    #[test]
    fn provider_parsing() {
        assert!(matches!(
            TokenizerSpec::from_params("OpenAI", None, None).unwrap(),
            TokenizerSpec::OpenAi { .. }
        ));
        assert!(matches!(
            TokenizerSpec::from_params("claude", None, None).unwrap(),
            TokenizerSpec::Anthropic
        ));
        assert!(matches!(
            TokenizerSpec::from_params("deepseek", None, None).unwrap(),
            TokenizerSpec::HuggingFace { .. }
        ));
        assert!(TokenizerSpec::from_params("nonsense", None, None).is_err());
    }

    #[test]
    fn long_text_stays_consistent() {
        let long = "word ".repeat(5000);
        let m = analyze_text(&long, &TokenizerSpec::OpenAi { model: "gpt-4o".into() }).unwrap();
        assert_eq!(m.words, 5000);
        assert_eq!(m.spaces, 5000);
        assert_eq!(m.chars, 25000);
        assert_eq!(m.word_freq, vec![("word".to_string(), 5000)]);
        assert!(m.tokens > 0 && m.exact);
    }

    #[test]
    fn token_display_escapes() {
        assert_eq!(display_token(" the"), "␣the");
        assert_eq!(display_token("\n"), "\\n");
        assert_eq!(display_token("Ġword"), "␣word");
        assert_eq!(display_token(""), "(empty)");
    }
}
