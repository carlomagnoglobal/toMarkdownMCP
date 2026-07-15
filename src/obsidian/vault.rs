use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::frontmatter;
use super::wikilink::{parse_wikilinks, WikiLink};

const MAX_DEPTH: usize = 20;
const MAX_FILES: usize = 10_000;

#[derive(Debug, Serialize)]
pub struct NoteMeta {
    /// Vault-relative path including extension, e.g. "folder/Note A.md".
    pub path: String,
    /// Filename without extension.
    pub title: String,
    #[serde(skip)]
    pub frontmatter: serde_yaml::Value,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    /// (level, text) in document order.
    pub headings: Vec<(u8, String)>,
    #[serde(skip)]
    pub mtime: SystemTime,
}

#[derive(Debug, Serialize)]
pub struct Backlink {
    /// Note containing the link.
    pub from: String,
    pub link: WikiLink,
    /// The full source line, for context.
    pub context: String,
}

#[derive(Debug, Default, Serialize)]
pub struct VaultIndex {
    #[serde(skip)]
    pub root: PathBuf,
    /// note path -> meta
    pub notes: HashMap<String, NoteMeta>,
    /// lowercase stem -> note paths sharing it
    pub by_stem: HashMap<String, Vec<String>>,
    /// lowercase alias -> note paths
    pub aliases: HashMap<String, Vec<String>>,
    /// note path -> outgoing links
    pub links: HashMap<String, Vec<WikiLink>>,
    /// note path -> inbound links
    pub backlinks: HashMap<String, Vec<Backlink>>,
    /// note path -> ^block-id -> 1-indexed line
    pub blocks: HashMap<String, HashMap<String, usize>>,
    /// tag -> note paths
    pub tags: HashMap<String, Vec<String>>,
    /// non-markdown files (images, pdfs, .canvas, ...), vault-relative
    pub attachments: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", content = "path")]
pub enum Resolution {
    Resolved(String),
    Ambiguous(Vec<String>),
    Broken,
}

/// Walk the vault, returning (markdown_files, other_files) as vault-relative paths.
pub fn vault_walk(root: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let mut notes = Vec::new();
    let mut others = Vec::new();
    let mut count = 0usize;
    walk_inner(root, root, 0, &mut count, &mut notes, &mut others)?;
    notes.sort();
    others.sort();
    Ok((notes, others))
}

fn walk_inner(
    root: &Path,
    dir: &Path,
    depth: usize,
    count: &mut usize,
    notes: &mut Vec<String>,
    others: &mut Vec<String>,
) -> Result<()> {
    if depth > MAX_DEPTH || *count >= MAX_FILES {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory {}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            walk_inner(root, &path, depth + 1, count, notes, others)?;
        } else {
            if name.starts_with('.') {
                continue;
            }
            *count += 1;
            if *count > MAX_FILES {
                return Ok(());
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            match path.extension().and_then(|e| e.to_str()) {
                Some("md") | Some("markdown") | Some("mdown") => notes.push(rel),
                _ => others.push(rel),
            }
        }
    }
    Ok(())
}

fn note_stem(rel_path: &str) -> String {
    let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name).to_string()
}

/// Strip `.md` (etc.) from a vault-relative path for link-target comparison.
fn path_sans_ext(rel_path: &str) -> &str {
    rel_path
        .strip_suffix(".md")
        .or_else(|| rel_path.strip_suffix(".markdown"))
        .or_else(|| rel_path.strip_suffix(".mdown"))
        .unwrap_or(rel_path)
}

/// Extract inline #tags (outside code) plus frontmatter tags.
fn extract_tags(body: &str, fm: &serde_yaml::Value) -> Vec<String> {
    let mut tags: Vec<String> = frontmatter::string_list(fm, "tags");
    let mut in_fence = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let bytes = line.as_bytes();
        for (i, _) in line.match_indices('#') {
            // Must be start-of-line or preceded by whitespace; next char alphanumeric
            if i > 0 && !bytes[i - 1].is_ascii_whitespace() {
                continue;
            }
            let rest = &line[i + 1..];
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '/' || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            let tag = &rest[..end];
            if !tag.is_empty() && tag.chars().any(|c| c.is_alphabetic()) {
                tags.push(tag.to_string());
            }
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

fn extract_headings(body: &str) -> Vec<(u8, String)> {
    let mut headings = Vec::new();
    let mut in_fence = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || !t.starts_with('#') {
            continue;
        }
        let level = t.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&level) && t.chars().nth(level) == Some(' ') {
            headings.push((level as u8, t[level + 1..].trim().to_string()));
        }
    }
    headings
}

/// ^block-id anchors: a trailing `^id` at the end of a line.
fn extract_blocks(body: &str) -> HashMap<String, usize> {
    let mut blocks = HashMap::new();
    for (i, line) in body.lines().enumerate() {
        if let Some(idx) = line.rfind('^') {
            let id = &line[idx + 1..];
            if !id.is_empty()
                && id.chars().all(|c| c.is_alphanumeric() || c == '-')
                && (idx == 0 || line.as_bytes()[idx - 1].is_ascii_whitespace())
            {
                blocks.insert(id.to_string(), i + 1);
            }
        }
    }
    blocks
}

/// Build a full index of the vault (stateless).
pub fn build_index(root: &Path) -> Result<VaultIndex> {
    if !root.is_dir() {
        return Err(anyhow!("Vault path is not a directory: {}", root.display()));
    }
    let (note_paths, attachments) = vault_walk(root)?;
    let mut idx = VaultIndex {
        root: root.to_path_buf(),
        attachments,
        ..Default::default()
    };

    // First pass: parse every note.
    let mut bodies: HashMap<String, String> = HashMap::new();
    for rel in &note_paths {
        let abs = root.join(rel);
        let content = match std::fs::read_to_string(&abs) {
            Ok(c) => c,
            Err(_) => continue, // unreadable/non-utf8: skip
        };
        let mtime = std::fs::metadata(&abs)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let fm = frontmatter::parse(&content);
        let (_, body) = frontmatter::split(&content);
        let title = note_stem(rel);
        let aliases = frontmatter::string_list(&fm, "aliases")
            .into_iter()
            .chain(frontmatter::string_list(&fm, "alias"))
            .collect::<Vec<_>>();
        let tags = extract_tags(body, &fm);
        let headings = extract_headings(body);

        idx.by_stem.entry(title.to_lowercase()).or_default().push(rel.clone());
        for a in &aliases {
            idx.aliases.entry(a.to_lowercase()).or_default().push(rel.clone());
        }
        for t in &tags {
            idx.tags.entry(t.clone()).or_default().push(rel.clone());
        }
        // Line numbers from body-relative parses are shifted to be absolute
        // within the file (frontmatter included).
        let fm_line_offset = content.lines().count().saturating_sub(body.lines().count());
        let blocks = extract_blocks(body)
            .into_iter()
            .map(|(id, line)| (id, line + fm_line_offset))
            .collect();
        let links = parse_wikilinks(body)
            .into_iter()
            .map(|mut l| {
                l.line += fm_line_offset;
                l
            })
            .collect();
        idx.blocks.insert(rel.clone(), blocks);
        idx.links.insert(rel.clone(), links);
        idx.notes.insert(
            rel.clone(),
            NoteMeta { path: rel.clone(), title, frontmatter: fm, aliases, tags, headings, mtime },
        );
        bodies.insert(rel.clone(), content);
    }

    // Second pass: resolve links -> backlinks.
    let mut backlinks: HashMap<String, Vec<Backlink>> = HashMap::new();
    for (from, links) in &idx.links {
        for link in links {
            if let Resolution::Resolved(target) = resolve_target(&idx, &link.target, Some(from)) {
                let context = bodies
                    .get(from)
                    .and_then(|c| c.lines().nth(link.line.saturating_sub(1)))
                    .map(|l| {
                        // link.line is relative to the body; adjust by frontmatter offset
                        l.to_string()
                    })
                    .unwrap_or_default();
                backlinks.entry(target).or_default().push(Backlink {
                    from: from.clone(),
                    link: link.clone(),
                    context,
                });
            }
        }
    }
    idx.backlinks = backlinks;
    Ok(idx)
}

/// Resolve a link target string using Obsidian's shortest-path rules.
pub fn resolve_target(idx: &VaultIndex, target: &str, from: Option<&str>) -> Resolution {
    let target = target.trim();
    if target.is_empty() {
        // Self-reference like [[#heading]]
        return match from {
            Some(f) => Resolution::Resolved(f.to_string()),
            None => Resolution::Broken,
        };
    }
    let lower = target.to_lowercase();

    // 1. Path-qualified: match against full note paths (with or without extension).
    if target.contains('/') {
        for path in idx.notes.keys() {
            let sans = path_sans_ext(path);
            if sans.eq_ignore_ascii_case(target) || path.eq_ignore_ascii_case(target) {
                return Resolution::Resolved(path.clone());
            }
        }
        // Attachments can be path-qualified too
        for att in &idx.attachments {
            if att.eq_ignore_ascii_case(target) {
                return Resolution::Resolved(att.clone());
            }
        }
        return Resolution::Broken;
    }

    // 2. Stem lookup.
    if let Some(candidates) = idx.by_stem.get(&lower) {
        return pick_candidate(candidates, from);
    }
    // Target may carry an extension already ([[Note.md]], [[img.png]]).
    if let Some(stripped) = lower.strip_suffix(".md") {
        if let Some(candidates) = idx.by_stem.get(stripped) {
            return pick_candidate(candidates, from);
        }
    }

    // 3. Alias lookup.
    if let Some(candidates) = idx.aliases.get(&lower) {
        return pick_candidate(candidates, from);
    }

    // 4. Attachment filename match (for embeds like ![[img.png]]).
    let att_matches: Vec<String> = idx
        .attachments
        .iter()
        .filter(|a| {
            a.rsplit('/').next().map(|n| n.eq_ignore_ascii_case(target)).unwrap_or(false)
        })
        .cloned()
        .collect();
    match att_matches.len() {
        1 => Resolution::Resolved(att_matches.into_iter().next().unwrap()),
        n if n > 1 => Resolution::Ambiguous(att_matches),
        _ => Resolution::Broken,
    }
}

fn pick_candidate(candidates: &[String], from: Option<&str>) -> Resolution {
    match candidates.len() {
        0 => Resolution::Broken,
        1 => Resolution::Resolved(candidates[0].clone()),
        _ => {
            // Prefer a note in the same folder as the source.
            if let Some(from) = from {
                let dir = from.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                if let Some(same) = candidates.iter().find(|c| {
                    c.rsplit_once('/').map(|(d, _)| d).unwrap_or("") == dir
                }) {
                    return Resolution::Resolved(same.clone());
                }
            }
            // Else the shallowest path wins if unique.
            let min_depth = candidates.iter().map(|c| c.matches('/').count()).min().unwrap();
            let shallow: Vec<&String> =
                candidates.iter().filter(|c| c.matches('/').count() == min_depth).collect();
            if shallow.len() == 1 {
                Resolution::Resolved(shallow[0].clone())
            } else {
                Resolution::Ambiguous(candidates.to_vec())
            }
        }
    }
}

// ---- Cache: per-vault index invalidated by an mtime/file-set quick scan ----

struct CachedIndex {
    index: Arc<VaultIndex>,
    /// Fingerprint of (path, mtime) for every file seen at build time.
    fingerprint: u64,
}

static INDEX_CACHE: Lazy<Mutex<HashMap<PathBuf, CachedIndex>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn vault_fingerprint(root: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    if let Ok((notes, others)) = vault_walk(root) {
        for rel in notes.iter().chain(others.iter()) {
            rel.hash(&mut hasher);
            if let Ok(meta) = std::fs::metadata(root.join(rel)) {
                if let Ok(m) = meta.modified() {
                    m.hash(&mut hasher);
                }
                meta.len().hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

/// Get a (possibly cached) index for the vault. Rebuilds when any file's
/// path/mtime/size changed since the cached build.
pub fn get_index(root: &Path) -> Result<Arc<VaultIndex>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("Vault path not found: {}", root.display()))?;
    let fp = vault_fingerprint(&root);
    let mut cache = INDEX_CACHE.lock().unwrap();
    if let Some(cached) = cache.get(&root) {
        if cached.fingerprint == fp {
            return Ok(Arc::clone(&cached.index));
        }
    }
    let index = Arc::new(build_index(&root)?);
    cache.insert(root, CachedIndex { index: Arc::clone(&index), fingerprint: fp });
    Ok(index)
}

/// Invalidate the cached index for a vault (call after tools that write).
pub fn invalidate(root: &Path) {
    if let Ok(root) = root.canonicalize() {
        INDEX_CACHE.lock().unwrap().remove(&root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_vault")
    }

    #[test]
    fn index_fixture_vault() {
        let idx = build_index(&fixture()).unwrap();
        assert!(idx.notes.contains_key("Note A.md"), "notes: {:?}", idx.notes.keys());
        assert!(idx.notes.contains_key("folder/Note A.md"));

        // Aliases parsed from frontmatter
        assert!(idx.aliases.contains_key("alpha"), "aliases: {:?}", idx.aliases.keys());

        // Block anchor indexed
        assert!(idx.blocks.get("Note B.md").and_then(|b| b.get("quote1")).is_some());

        // Backlinks: Note B is linked from Note A (including via alias)
        let bl = idx.backlinks.get("Note B.md").expect("backlinks for Note B");
        assert!(bl.iter().any(|b| b.from == "Note A.md"));

        // Link in code fence ignored
        let a_links = &idx.links["Note A.md"];
        assert!(!a_links.iter().any(|l| l.target == "NotALink"));

        // Attachments include canvas + image
        assert!(idx.attachments.iter().any(|a| a.ends_with(".canvas")));
    }

    #[test]
    fn resolution_rules() {
        let idx = build_index(&fixture()).unwrap();

        // Ambiguous stem, disambiguated by same-folder preference
        match resolve_target(&idx, "Note A", Some("folder/Other.md")) {
            Resolution::Resolved(p) => assert_eq!(p, "folder/Note A.md"),
            other => panic!("expected same-folder resolution, got {:?}", other),
        }
        // From root note, shallowest wins
        match resolve_target(&idx, "Note A", Some("Note B.md")) {
            Resolution::Resolved(p) => assert_eq!(p, "Note A.md"),
            other => panic!("expected shallow resolution, got {:?}", other),
        }
        // Path-qualified
        match resolve_target(&idx, "folder/Note A", None) {
            Resolution::Resolved(p) => assert_eq!(p, "folder/Note A.md"),
            other => panic!("expected path resolution, got {:?}", other),
        }
        // Alias
        match resolve_target(&idx, "Alpha", None) {
            Resolution::Resolved(p) => assert_eq!(p, "Note A.md"),
            other => panic!("expected alias resolution, got {:?}", other),
        }
        // Broken
        assert!(matches!(resolve_target(&idx, "No Such Note", None), Resolution::Broken));
    }
}
