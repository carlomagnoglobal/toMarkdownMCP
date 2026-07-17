//! Document intelligence: readability scoring, natural-language detection, and
//! heuristic topic/content-type classification. All local and deterministic —
//! no ML models or data files. Reuses the shared tokenizer from `rag`.

use crate::rag::tokenize_words;

// ----------------------------------------------------------------------------
// Readability (Flesch)
// ----------------------------------------------------------------------------

pub struct Readability {
    pub words: usize,
    pub sentences: usize,
    pub syllables: usize,
    pub flesch_reading_ease: f64,
    pub flesch_kincaid_grade: f64,
    pub avg_sentence_length: f64,
}

/// Count syllables in an English word via a vowel-group heuristic.
pub fn count_syllables(word: &str) -> usize {
    let w = word.to_lowercase();
    let chars: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
    if chars.is_empty() {
        return 0;
    }
    let is_vowel = |c: char| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
    let mut count: usize = 0;
    let mut prev_vowel = false;
    for &c in &chars {
        let v = is_vowel(c);
        if v && !prev_vowel {
            count += 1;
        }
        prev_vowel = v;
    }
    // Silent trailing 'e' (e.g. "make"), but not a consonant + "-le" ending
    // which forms its own syllable (e.g. "apple", "table").
    let n = chars.len();
    if n > 2 && chars[n - 1] == 'e' && !is_vowel(chars[n - 2]) && chars[n - 2] != 'l' {
        count = count.saturating_sub(1);
    }
    count.max(1)
}

/// Compute Flesch Reading Ease and Flesch-Kincaid Grade Level for text.
pub fn readability(text: &str) -> Readability {
    let words: Vec<String> = tokenize_words(text);
    let word_count = words.len().max(1);
    let sentences = text
        .split(['.', '!', '?'])
        .filter(|s| !s.trim().is_empty())
        .count()
        .max(1);
    let syllables: usize = words.iter().map(|w| count_syllables(w)).sum::<usize>().max(1);

    let wps = word_count as f64 / sentences as f64; // words per sentence
    let spw = syllables as f64 / word_count as f64; // syllables per word

    let flesch_reading_ease = 206.835 - 1.015 * wps - 84.6 * spw;
    let flesch_kincaid_grade = 0.39 * wps + 11.8 * spw - 15.59;

    Readability {
        words: word_count,
        sentences,
        syllables,
        flesch_reading_ease,
        flesch_kincaid_grade,
        avg_sentence_length: wps,
    }
}

/// Human-readable interpretation of a Flesch Reading Ease score.
pub fn flesch_interpretation(score: f64) -> &'static str {
    match score as i64 {
        90..=i64::MAX => "Very easy (5th grade)",
        80..=89 => "Easy (6th grade)",
        70..=79 => "Fairly easy (7th grade)",
        60..=69 => "Standard (8th-9th grade)",
        50..=59 => "Fairly difficult (10th-12th grade)",
        30..=49 => "Difficult (college)",
        _ => "Very difficult (college graduate)",
    }
}

// ----------------------------------------------------------------------------
// Natural-language detection
// ----------------------------------------------------------------------------

/// Common function words per language, used for stopword-hit-rate detection.
fn language_markers() -> Vec<(&'static str, &'static [&'static str])> {
    vec![
        ("English", &["the", "and", "of", "to", "in", "is", "that", "it", "for", "with", "as", "was"]),
        ("Spanish", &["el", "la", "de", "que", "y", "los", "las", "en", "un", "una", "por", "con"]),
        ("French", &["le", "la", "de", "et", "les", "des", "un", "une", "que", "dans", "pour", "est"]),
        ("German", &["der", "die", "das", "und", "den", "ein", "eine", "ist", "mit", "auf", "für", "nicht"]),
        ("Portuguese", &["o", "a", "de", "que", "e", "do", "da", "em", "um", "uma", "para", "com"]),
        ("Italian", &["il", "la", "di", "che", "e", "un", "una", "in", "per", "con", "non", "sono"]),
    ]
}

pub struct LanguageGuess {
    pub language: &'static str,
    pub confidence: f64,
}

/// Rank likely natural languages by function-word hit rate.
pub fn detect_language(text: &str) -> Vec<LanguageGuess> {
    let tokens = tokenize_words(text);
    let total = tokens.len().max(1) as f64;
    let token_set: std::collections::HashSet<&str> = tokens.iter().map(|t| t.as_str()).collect();

    let mut guesses: Vec<LanguageGuess> = language_markers()
        .into_iter()
        .map(|(lang, markers)| {
            let hits: usize = tokens.iter().filter(|t| markers.contains(&t.as_str())).count();
            // Weight by both frequency and coverage of distinct markers.
            let coverage = markers.iter().filter(|m| token_set.contains(*m)).count() as f64
                / markers.len() as f64;
            let confidence = (hits as f64 / total) * 0.7 + coverage * 0.3;
            LanguageGuess { language: lang, confidence }
        })
        .collect();

    guesses.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    guesses
}

// ----------------------------------------------------------------------------
// Topic / content-type classification
// ----------------------------------------------------------------------------

/// Keyword signal sets per content category.
fn category_signals() -> Vec<(&'static str, &'static [&'static str])> {
    vec![
        ("technical/code", &["function", "class", "return", "import", "variable", "code", "api", "method", "compile", "async", "struct", "def"]),
        ("finance", &["revenue", "profit", "cost", "budget", "invoice", "payment", "tax", "financial", "quarter", "expense", "balance", "account"]),
        ("legal", &["agreement", "party", "hereby", "clause", "liability", "terms", "contract", "law", "shall", "pursuant", "jurisdiction", "warranty"]),
        ("correspondence", &["dear", "regards", "sincerely", "hello", "thanks", "email", "reply", "meeting", "subject", "sent", "message", "hi"]),
        ("academic", &["research", "study", "hypothesis", "results", "conclusion", "abstract", "method", "analysis", "figure", "citation", "experiment", "data"]),
        ("marketing", &["product", "customer", "brand", "campaign", "launch", "market", "sales", "offer", "audience", "growth", "engagement", "conversion"]),
    ]
}

pub struct CategoryScore {
    pub category: &'static str,
    pub score: f64,
}

/// Rank content categories by keyword-signal hit rate.
pub fn classify(text: &str) -> Vec<CategoryScore> {
    let tokens = tokenize_words(text);
    let total = tokens.len().max(1) as f64;

    let mut scores: Vec<CategoryScore> = category_signals()
        .into_iter()
        .map(|(cat, signals)| {
            let hits: usize = tokens.iter().filter(|t| signals.contains(&t.as_str())).count();
            CategoryScore { category: cat, score: (hits as f64 / total) * 100.0 }
        })
        .filter(|c| c.score > 0.0)
        .collect();

    scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_syllables() {
        assert_eq!(count_syllables("cat"), 1);
        assert_eq!(count_syllables("apple"), 2);
        assert_eq!(count_syllables("readability"), 5);
        assert_eq!(count_syllables("make"), 1); // silent e
    }

    #[test]
    fn test_readability_simple_vs_complex() {
        let simple = readability("The cat sat on the mat. The dog ran fast.");
        let complex = readability("Consequently, the aforementioned methodology necessitates comprehensive evaluation.");
        assert!(simple.flesch_reading_ease > complex.flesch_reading_ease);
    }

    #[test]
    fn test_detect_language_english_vs_spanish() {
        let en = detect_language("the quick brown fox and the lazy dog is in the house");
        assert_eq!(en[0].language, "English");
        let es = detect_language("el perro y el gato de la casa que es un animal");
        assert_eq!(es[0].language, "Spanish");
    }

    #[test]
    fn test_classify_technical() {
        let scores = classify("This function returns a value. Import the class and call the method in your code.");
        assert_eq!(scores[0].category, "technical/code");
    }
}
