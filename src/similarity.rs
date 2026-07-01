//! Near-duplicate detection (SimHash) and topic clustering (cosine over term
//! frequencies). Pure algorithms; file IO lives in the handlers in main.rs.

use std::collections::HashMap;

use crate::knowledge::{cosine_similarity, term_frequencies};
use crate::rag::tokenize_words;

// ----------------------------------------------------------------------------
// SimHash near-duplicate detection
// ----------------------------------------------------------------------------

/// Compute a 64-bit SimHash fingerprint of a document over word tokens.
pub fn simhash(text: &str) -> u64 {
    let tokens = tokenize_words(text);
    if tokens.is_empty() {
        return 0;
    }
    let mut counts: HashMap<String, i64> = HashMap::new();
    for t in tokens {
        *counts.entry(t).or_insert(0) += 1;
    }

    let mut vector = [0i64; 64];
    for (token, weight) in counts {
        let h = fnv1a_64(token.as_bytes());
        for (bit, v) in vector.iter_mut().enumerate() {
            if (h >> bit) & 1 == 1 {
                *v += weight;
            } else {
                *v -= weight;
            }
        }
    }

    let mut fingerprint = 0u64;
    for (bit, &v) in vector.iter().enumerate() {
        if v > 0 {
            fingerprint |= 1 << bit;
        }
    }
    fingerprint
}

/// FNV-1a 64-bit hash — fast, dependency-free, good enough for SimHash.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Hamming distance between two fingerprints (number of differing bits).
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Similarity in [0,1] derived from Hamming distance over 64 bits.
pub fn simhash_similarity(a: u64, b: u64) -> f64 {
    1.0 - (hamming_distance(a, b) as f64 / 64.0)
}

/// Group items whose SimHash Hamming distance is <= `threshold` bits. Returns
/// only groups with more than one member (i.e. actual near-duplicate sets).
pub fn group_near_duplicates(items: &[(String, u64)], threshold: u32) -> Vec<Vec<(String, f64)>> {
    let n = items.len();
    let mut visited = vec![false; n];
    let mut groups = Vec::new();

    for i in 0..n {
        if visited[i] {
            continue;
        }
        let mut group = vec![(items[i].0.clone(), 1.0)];
        visited[i] = true;
        for j in (i + 1)..n {
            if visited[j] {
                continue;
            }
            if hamming_distance(items[i].1, items[j].1) <= threshold {
                visited[j] = true;
                group.push((items[j].0.clone(), simhash_similarity(items[i].1, items[j].1)));
            }
        }
        if group.len() > 1 {
            groups.push(group);
        }
    }
    groups
}

// ----------------------------------------------------------------------------
// Cosine clustering
// ----------------------------------------------------------------------------

pub struct Document {
    pub path: String,
    pub tf: HashMap<String, usize>,
}

pub struct Cluster {
    pub label: String,
    pub members: Vec<String>,
    pub top_terms: Vec<String>,
}

impl Document {
    pub fn new(path: &str, text: &str) -> Self {
        Document {
            path: path.to_string(),
            tf: term_frequencies(text),
        }
    }
}

/// Greedy agglomerative clustering: each document joins the first existing
/// cluster whose centroid similarity meets `min_similarity`, else starts a new
/// cluster. The cluster label + top terms come from the merged term frequencies.
pub fn cluster_documents(docs: &[Document], min_similarity: f64) -> Vec<Cluster> {
    let mut centroids: Vec<HashMap<String, usize>> = Vec::new();
    let mut members: Vec<Vec<String>> = Vec::new();

    for doc in docs {
        let mut best = None;
        let mut best_sim = min_similarity;
        for (idx, centroid) in centroids.iter().enumerate() {
            let sim = cosine_similarity(&doc.tf, centroid);
            if sim >= best_sim {
                best_sim = sim;
                best = Some(idx);
            }
        }
        match best {
            Some(idx) => {
                for (term, count) in &doc.tf {
                    *centroids[idx].entry(term.clone()).or_insert(0) += count;
                }
                members[idx].push(doc.path.clone());
            }
            None => {
                centroids.push(doc.tf.clone());
                members.push(vec![doc.path.clone()]);
            }
        }
    }

    centroids
        .into_iter()
        .zip(members)
        .map(|(centroid, members)| {
            let top_terms = top_terms(&centroid, 5);
            let label = top_terms.first().cloned().unwrap_or_else(|| "misc".to_string());
            Cluster { label, members, top_terms }
        })
        .collect()
}

/// Highest-frequency terms of a term map.
fn top_terms(tf: &HashMap<String, usize>, n: usize) -> Vec<String> {
    let mut v: Vec<(&String, &usize)> = tf.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    v.into_iter().take(n).map(|(t, _)| t.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simhash_identical_and_different() {
        let a = simhash("the quick brown fox jumps over the lazy dog");
        let b = simhash("the quick brown fox jumps over the lazy dog");
        let c = simhash("completely unrelated content about cooking and food recipes");
        assert_eq!(hamming_distance(a, b), 0);
        assert!(hamming_distance(a, c) > 10);
    }

    #[test]
    fn test_group_near_duplicates() {
        let items = vec![
            ("a".to_string(), simhash("rust systems programming language memory safety")),
            ("b".to_string(), simhash("rust systems programming language memory safety!")),
            ("c".to_string(), simhash("baking bread requires flour water yeast and salt")),
        ];
        let groups = group_near_duplicates(&items, 3);
        assert_eq!(groups.len(), 1);
        let paths: Vec<&String> = groups[0].iter().map(|(p, _)| p).collect();
        assert!(paths.contains(&&"a".to_string()) && paths.contains(&&"b".to_string()));
    }

    #[test]
    fn test_cluster_documents_groups_similar() {
        let docs = vec![
            Document::new("r1", "rust ownership borrowing lifetimes memory"),
            Document::new("r2", "rust memory ownership traits generics"),
            Document::new("f1", "recipe flour sugar butter baking oven"),
            Document::new("f2", "baking bread flour water yeast recipe"),
        ];
        let clusters = cluster_documents(&docs, 0.15);
        // Expect roughly two clusters (rust vs baking).
        assert!(clusters.len() >= 2);
        // The two rust docs should share a cluster.
        let rust_cluster = clusters.iter().find(|c| c.members.contains(&"r1".to_string())).unwrap();
        assert!(rust_cluster.members.contains(&"r2".to_string()));
    }
}
