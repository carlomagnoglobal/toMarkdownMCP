//! Vector embeddings for the RAG tools, behind the opt-in `embeddings: true`
//! tool parameter. Two backends implement [`Embedder`]:
//!
//! - `FastEmbedder` (cargo feature `embeddings`): real sentence embeddings via
//!   fastembed/ONNX (all-MiniLM-L6-v2, 384 dims; downloads the model on first
//!   use).
//! - `HashEmbedder` (always available): deterministic hashed bag-of-words
//!   vectors — no model, no network. Used whenever the real backend is
//!   unavailable, so `embeddings: true` degrades instead of failing.
//!
//! A [`VectorIndex`] persists chunk embeddings per directory under
//! `.tomarkdown/embeddings_index.json`, re-embedding only files whose mtime
//! changed since the last run.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::rag;

pub trait Embedder {
    /// Identifier stored in the index; a model change invalidates it.
    fn model_id(&self) -> &str;
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// Deterministic hashed bag-of-words embedding: each word hashes into one of
/// `HASH_DIM` buckets, the vector is L2-normalized. No model required; cosine
/// over these approximates TF-vector similarity.
pub struct HashEmbedder;

const HASH_DIM: usize = 512;

impl Embedder for HashEmbedder {
    fn model_id(&self) -> &str {
        "hash-bow-512"
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| hash_embed(t)).collect())
    }
}

fn hash_embed(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; HASH_DIM];
    for word in rag::tokenize_words(text) {
        let mut h = DefaultHasher::new();
        word.hash(&mut h);
        v[(h.finish() as usize) % HASH_DIM] += 1.0;
    }
    l2_normalize(&mut v);
    v
}

fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    // Vectors from Embedder backends are already L2-normalized.
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(feature = "embeddings")]
pub struct FastEmbedder {
    model: fastembed::TextEmbedding,
}

#[cfg(feature = "embeddings")]
impl FastEmbedder {
    pub fn new() -> Result<Self> {
        let model = fastembed::TextEmbedding::try_new(
            fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2),
        )?;
        Ok(Self { model })
    }
}

#[cfg(feature = "embeddings")]
impl Embedder for FastEmbedder {
    fn model_id(&self) -> &str {
        "all-minilm-l6-v2"
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let mut out = self.model.embed(refs, None)?;
        for v in &mut out {
            l2_normalize(v);
        }
        Ok(out)
    }
}

/// Best available embedder: the fastembed model when the feature is compiled
/// in and the model loads (first use downloads it), otherwise the hash
/// fallback with a stderr notice.
pub fn default_embedder() -> Box<dyn Embedder> {
    #[cfg(feature = "embeddings")]
    {
        match FastEmbedder::new() {
            Ok(e) => return Box::new(e),
            Err(e) => eprintln!(
                "embeddings: model unavailable ({}); falling back to hashed vectors",
                e
            ),
        }
    }
    Box::new(HashEmbedder)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChunkEmbedding {
    pub heading_path: Vec<String>,
    pub text: String,
    pub token_estimate: usize,
    pub vector: Vec<f32>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct FileEntry {
    pub mtime_secs: u64,
    pub chunks: Vec<ChunkEmbedding>,
}

/// On-disk chunk-embedding index for one directory.
#[derive(Serialize, Deserialize, Default)]
pub struct VectorIndex {
    pub model: String,
    pub files: BTreeMap<String, FileEntry>,
}

fn index_path(dir: &Path) -> PathBuf {
    dir.join(".tomarkdown").join("embeddings_index.json")
}

fn mtime_secs(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl VectorIndex {
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(index_path(dir))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let path = index_path(dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string(self)?)?;
        Ok(())
    }

    /// Bring the index up to date for `sources` (paths with their converted
    /// Markdown supplied by `read`), embedding only new/changed files and
    /// dropping entries whose file is gone. Returns how many files were
    /// (re-)embedded.
    pub fn update(
        &mut self,
        sources: &[PathBuf],
        embedder: &mut dyn Embedder,
        read: impl Fn(&Path) -> Option<String>,
    ) -> Result<usize> {
        if self.model != embedder.model_id() {
            // Different model: every stored vector is stale.
            self.files.clear();
            self.model = embedder.model_id().to_string();
        }
        let mut refreshed = 0;
        let keep: std::collections::BTreeSet<String> =
            sources.iter().map(|p| p.display().to_string()).collect();
        self.files.retain(|k, _| keep.contains(k));
        for path in sources {
            let key = path.display().to_string();
            let mtime = mtime_secs(path);
            if self.files.get(&key).is_some_and(|e| e.mtime_secs == mtime) {
                continue;
            }
            let Some(content) = read(path) else { continue };
            let chunks = rag::chunk_markdown(&content, 512, 64);
            let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
            let vectors = embedder.embed(&texts)?;
            let entry = FileEntry {
                mtime_secs: mtime,
                chunks: chunks
                    .into_iter()
                    .zip(vectors)
                    .map(|(c, vector)| ChunkEmbedding {
                        heading_path: c.heading_path,
                        text: c.text,
                        token_estimate: c.token_estimate,
                        vector,
                    })
                    .collect(),
            };
            self.files.insert(key, entry);
            refreshed += 1;
        }
        Ok(refreshed)
    }

    /// Chunks ranked by cosine similarity to `query_vector`, best first.
    pub fn rank(&self, query_vector: &[f32]) -> Vec<(String, &ChunkEmbedding, f32)> {
        let mut scored: Vec<(String, &ChunkEmbedding, f32)> = self
            .files
            .iter()
            .flat_map(|(path, entry)| {
                entry
                    .chunks
                    .iter()
                    .map(move |c| (path.clone(), c, cosine(query_vector, &c.vector)))
            })
            .collect();
        scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    /// One vector per file: the L2-normalized mean of its chunk vectors.
    pub fn file_vectors(&self) -> Vec<(String, Vec<f32>)> {
        self.files
            .iter()
            .filter_map(|(path, entry)| {
                let first = entry.chunks.first()?;
                let dim = first.vector.len();
                let mut mean = vec![0f32; dim];
                for c in &entry.chunks {
                    for (m, x) in mean.iter_mut().zip(&c.vector) {
                        *m += x;
                    }
                }
                l2_normalize(&mut mean);
                Some((path.clone(), mean))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_embedder_is_deterministic_and_normalized() {
        let mut e = HashEmbedder;
        let v = e.embed(&["hello world hello".to_string()]).unwrap();
        let v2 = e.embed(&["hello world hello".to_string()]).unwrap();
        assert_eq!(v, v2);
        let norm: f32 = v[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_prefers_similar_text() {
        let mut e = HashEmbedder;
        let vs = e
            .embed(&[
                "rust programming language memory safety".to_string(),
                "rust language for safe systems programming".to_string(),
                "chocolate cake baking recipe with sugar".to_string(),
            ])
            .unwrap();
        assert!(cosine(&vs[0], &vs[1]) > cosine(&vs[0], &vs[2]));
    }

    #[test]
    fn index_updates_incrementally() {
        let dir = std::env::temp_dir().join(format!("emb_idx_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.md");
        let b = dir.join("b.md");
        std::fs::write(&a, "# Alpha\n\nrust memory safety").unwrap();
        std::fs::write(&b, "# Beta\n\nbaking chocolate cake").unwrap();

        let mut index = VectorIndex::load(&dir);
        let mut embedder = HashEmbedder;
        let sources = vec![a.clone(), b.clone()];
        let read = |p: &Path| std::fs::read_to_string(p).ok();

        let n = index.update(&sources, &mut embedder, read).unwrap();
        assert_eq!(n, 2);
        // Unchanged files are skipped on the next update.
        let n = index.update(&sources, &mut embedder, read).unwrap();
        assert_eq!(n, 0);
        // Persisted and reloaded index still knows both files.
        index.save(&dir).unwrap();
        let reloaded = VectorIndex::load(&dir);
        assert_eq!(reloaded.files.len(), 2);
        // Removing a source drops its entry.
        let mut index = reloaded;
        let n = index.update(std::slice::from_ref(&a), &mut embedder, read).unwrap();
        assert_eq!(n, 0);
        assert_eq!(index.files.len(), 1);
        // Ranking finds the relevant chunk first.
        let qv = embedder.embed(&["rust safety".to_string()]).unwrap();
        let ranked = index.rank(&qv[0]);
        assert!(ranked[0].0.ends_with("a.md"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
