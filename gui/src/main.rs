//! Desktop viewer for toMarkdownMCP: file tree + rendered Markdown pane.
//! All conversion/vault logic comes from the `to_markdown_mcp` library; this
//! crate only adds Tauri commands and Markdown→HTML rendering for the webview.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use notify::Watcher;
use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

use to_markdown_mcp::file_type::{detect_language, detect_language_from_filename};
use to_markdown_mcp::obsidian::{tools as vault_tools, vault};
use to_markdown_mcp::pipeline::convert_any_to_markdown;

mod render;
use render::{render_note, RenderOpts};

#[derive(Serialize)]
struct TreeNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Vec<TreeNode>,
}

const EXCLUDED_DIRS: &[&str] = &[
    "node_modules", "target", ".git", "__pycache__", ".venv", "dist", "build",
    ".obsidian", ".tomarkdown",
];

fn build_tree(dir: &Path, depth: usize) -> Vec<TreeNode> {
    if depth > 12 {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut nodes: Vec<TreeNode> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || EXCLUDED_DIRS.contains(&name.as_str()) {
                return None;
            }
            let is_dir = path.is_dir();
            Some(TreeNode {
                children: if is_dir { build_tree(&path, depth + 1) } else { Vec::new() },
                name,
                path: path.to_string_lossy().into_owned(),
                is_dir,
            })
        })
        .collect();
    // Directories first, then case-insensitive by name — like a file manager.
    nodes.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    nodes
}

#[tauri::command]
async fn list_tree(root: String) -> Result<Vec<TreeNode>, String> {
    let path = PathBuf::from(&root);
    if !path.is_dir() {
        return Err(format!("Not a directory: {}", root));
    }
    Ok(build_tree(&path, 0))
}

#[derive(Serialize)]
struct Rendered {
    title: String,
    html: String,
    words: usize,
    chars: usize,
    read_minutes: usize,
}

#[tauri::command]
async fn open_file(path: String, vault_root: Option<String>) -> Result<Rendered, String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("Not a file: {}", path));
    }
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    let opts = RenderOpts {
        file_dir: p.parent(),
        vault_root: vault_root.as_deref().map(Path::new),
    };
    // Obsidian canvas files render via the JsonCanvas→Markdown converter.
    if ext == "canvas" {
        let value = vault_tools::convert_canvas(&p).map_err(|e| e.to_string())?;
        let md = value["markdown"].as_str().unwrap_or_default().to_string();
        let words = md.split_whitespace().count();
        return Ok(Rendered {
            title: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path),
            html: render_note(&md, &opts, 0),
            words,
            chars: md.chars().count(),
            read_minutes: (words / 200).max(1),
        });
    }
    let converted = convert_any_to_markdown(&p).map_err(|e| e.to_string())?;
    let words = converted.split_whitespace().count();
    let chars = converted.chars().count();
    // Markdown-ish output renders directly; code/text gets a fenced block so
    // the viewer shows it monospaced.
    let md = if matches!(ext, "md" | "markdown") || to_markdown_mcp::pipeline::is_structured_ext(Some(ext)) {
        converted
    } else {
        let lang = {
            let detected = detect_language(&p);
            if detected.is_empty() {
                detect_language_from_filename(p.file_name().and_then(|n| n.to_str()).unwrap_or(""))
            } else {
                detected
            }
        };
        format!("```{}\n{}\n```\n", lang, converted)
    };
    Ok(Rendered {
        title: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path),
        html: render_note(&md, &opts, 0),
        words,
        chars,
        // ~200 words/minute, floor 1 so a short note doesn't show "0 min".
        read_minutes: (words / 200).max(1),
    })
}

/// Live watchers: one for the open folder tree, one for the parent of the
/// open file (editors replace files on save, so the file itself would lose
/// its inode). Setting a new watcher drops and replaces the previous one.
#[derive(Default)]
struct WatchState {
    tree: Mutex<Option<notify::RecommendedWatcher>>,
    file: Mutex<Option<notify::RecommendedWatcher>>,
}

fn make_watcher(
    app: tauri::AppHandle,
    event_name: &'static str,
    target: &Path,
    mode: notify::RecursiveMode,
) -> Result<notify::RecommendedWatcher, String> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            // Only content-affecting events; skip pure access notifications.
            if matches!(
                event.kind,
                notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)
            ) {
                let paths: Vec<String> =
                    event.paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
                let _ = app.emit(event_name, paths);
            }
        }
    })
    .map_err(|e| e.to_string())?;
    watcher.watch(target, mode).map_err(|e| e.to_string())?;
    Ok(watcher)
}

#[tauri::command]
fn watch_tree(
    app: tauri::AppHandle,
    state: tauri::State<WatchState>,
    root: String,
) -> Result<(), String> {
    let watcher = make_watcher(app, "tree-changed", Path::new(&root), notify::RecursiveMode::Recursive)?;
    *state.tree.lock().unwrap() = Some(watcher);
    Ok(())
}

#[tauri::command]
fn watch_file(
    app: tauri::AppHandle,
    state: tauri::State<WatchState>,
    path: String,
) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let parent = p.parent().unwrap_or(Path::new(".")).to_path_buf();
    let watcher = make_watcher(app, "file-changed", &parent, notify::RecursiveMode::NonRecursive)?;
    *state.file.lock().unwrap() = Some(watcher);
    Ok(())
}

/// Save a standalone styled HTML export of the current document.
#[tauri::command]
async fn export_html(
    app: tauri::AppHandle,
    title: String,
    body_html: String,
    css: String,
) -> Result<Option<String>, String> {
    let Some(dest) = app
        .dialog()
        .file()
        .set_file_name(format!("{}.html", title.trim_end_matches(".md")))
        .add_filter("HTML", &["html"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = dest.into_path().map_err(|e| e.to_string())?;
    let doc = format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n<style>\n{}\n</style>\n</head>\n<body>\n<main class=\"content\">\n{}\n</main>\n</body>\n</html>\n",
        title, css, body_html
    );
    std::fs::write(&path, doc).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

/// Vault-relative path of `abs` under `root`, if it is inside the vault.
fn vault_rel(root: &str, abs: &str) -> Option<String> {
    let root = PathBuf::from(root).canonicalize().ok()?;
    let abs = PathBuf::from(abs).canonicalize().ok()?;
    abs.strip_prefix(&root).ok().map(|p| p.to_string_lossy().into_owned())
}

/// Everything the note panel needs for the open file: properties
/// (frontmatter), tags, headings, outgoing links, backlinks, Dataview fields.
#[tauri::command]
fn note_info(root: String, path: String) -> Result<serde_json::Value, String> {
    let rel = vault_rel(&root, &path).ok_or("File is outside the vault")?;
    let rootp = Path::new(&root);
    let note = vault_tools::get_note(rootp, &rel, false, 0).map_err(|e| e.to_string())?;
    let backlinks = vault_tools::get_backlinks(rootp, &rel).map_err(|e| e.to_string())?;
    let fields = vault_tools::extract_dataview_fields(rootp, Some(&rel), None).unwrap_or_default();
    Ok(serde_json::json!({ "note": note, "backlinks": backlinks, "fields": fields }))
}

#[tauri::command]
async fn vault_overview(root: String) -> Result<serde_json::Value, String> {
    vault_tools::vault_index(Path::new(&root), true).map_err(|e| e.to_string())
}

#[tauri::command]
async fn vault_search(root: String, query: String, mode: String) -> Result<serde_json::Value, String> {
    vault_tools::search(Path::new(&root), &query, &mode, 50).map_err(|e| e.to_string())
}

#[tauri::command]
async fn vault_tasks(root: String) -> Result<serde_json::Value, String> {
    vault_tools::list_tasks(Path::new(&root), None, None).map_err(|e| e.to_string())
}

/// Resolve a wikilink target to an absolute path, using Obsidian's
/// shortest-path rules relative to the note the click came from.
#[tauri::command]
fn resolve_wikilink(root: String, target: String, from: String) -> Result<String, String> {
    let idx = vault::get_index(Path::new(&root)).map_err(|e| e.to_string())?;
    let from_rel = vault_rel(&root, &from);
    match vault::resolve_target(&idx, &target, from_rel.as_deref()) {
        vault::Resolution::Resolved(rel) => Ok(Path::new(&root).join(rel).to_string_lossy().into_owned()),
        vault::Resolution::Ambiguous(mut candidates) => {
            // Obsidian opens the first shortest-path candidate.
            candidates.sort_by_key(|c| c.len());
            candidates
                .first()
                .map(|rel| Path::new(&root).join(rel).to_string_lossy().into_owned())
                .ok_or_else(|| format!("Ambiguous link with no candidates: {}", target))
        }
        vault::Resolution::Broken => Err(format!("Broken link: {}", target)),
    }
}

/// Nodes and links for the vault graph view. `focus` narrows to the direct
/// neighborhood of one note (local graph).
#[tauri::command]
async fn graph_data(root: String, focus: Option<String>) -> Result<serde_json::Value, String> {
    let idx = vault::get_index(Path::new(&root)).map_err(|e| e.to_string())?;
    let mut edges: Vec<(String, String)> = Vec::new();
    for (from, links) in &idx.links {
        for link in links {
            if let vault::Resolution::Resolved(to) = vault::resolve_target(&idx, &link.target, Some(from)) {
                edges.push((from.clone(), to));
            }
        }
    }
    let focus_rel = focus.as_deref().and_then(|f| vault_rel(&root, f));
    if let Some(center) = &focus_rel {
        edges.retain(|(a, b)| a == center || b == center);
    }
    let mut node_set: std::collections::BTreeSet<String> = edges
        .iter()
        .flat_map(|(a, b)| [a.clone(), b.clone()])
        .collect();
    match &focus_rel {
        Some(center) => {
            node_set.insert(center.clone());
        }
        // The global graph also shows unlinked notes.
        None => node_set.extend(idx.notes.keys().cloned()),
    }
    let nodes: Vec<serde_json::Value> = node_set
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p,
                "title": idx.notes.get(p).map(|n| n.title.clone()).unwrap_or_else(|| p.clone()),
                "links": edges.iter().filter(|(a, b)| a == p || b == p).count(),
            })
        })
        .collect();
    let links: Vec<serde_json::Value> = edges
        .iter()
        .map(|(a, b)| serde_json::json!({ "source": a, "target": b }))
        .collect();
    Ok(serde_json::json!({ "nodes": nodes, "links": links }))
}

/// Titles + aliases for the quick switcher's fuzzy matching.
#[tauri::command]
fn quick_list(root: String) -> Result<serde_json::Value, String> {
    let idx = vault::get_index(Path::new(&root)).map_err(|e| e.to_string())?;
    let mut items: Vec<serde_json::Value> = idx
        .notes
        .values()
        .map(|n| serde_json::json!({ "path": n.path, "title": n.title, "aliases": n.aliases }))
        .collect();
    items.sort_by(|a, b| a["title"].as_str().cmp(&b["title"].as_str()));
    Ok(serde_json::json!(items))
}

// ---- Editing commands (Phase 8) ----

/// Raw source for the editor, capped at the structured-conversion limit.
#[tauri::command]
fn read_source(path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    let size = std::fs::metadata(&p).map(|m| m.len()).map_err(|e| e.to_string())?;
    if size > to_markdown_mcp::pipeline::LARGE_FILE_BYTES {
        return Err("File too large to edit here".to_string());
    }
    std::fs::read_to_string(&p).map_err(|e| e.to_string())
}

/// Atomic save: write a temp file next to the target, then rename over it.
/// Invalidates the vault index so links/backlinks stay fresh.
#[tauri::command]
fn save_file(path: String, content: String, vault_root: Option<String>) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let tmp = p.with_extension("tomarkdown.tmp");
    std::fs::write(&tmp, &content).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    if let Some(root) = vault_root {
        vault::invalidate(Path::new(&root));
    }
    Ok(())
}

/// Render markdown for the edit-mode live preview (same pipeline as open_file).
#[tauri::command]
fn render_markdown(md: String, vault_root: Option<String>, file_path: Option<String>) -> String {
    let file_dir = file_path.as_deref().and_then(|p| Path::new(p).parent().map(Path::to_path_buf));
    let opts = RenderOpts {
        file_dir: file_dir.as_deref(),
        vault_root: vault_root.as_deref().map(Path::new),
    };
    render_note(&md, &opts, 0)
}

/// Class-based syntect CSS for both themes; injected once by the frontend.
#[tauri::command]
fn syntax_css() -> String {
    render::syntax_css()
}

/// Toggle the nth checkbox task in a file (0-indexed, document order),
/// matching how the rendered preview orders its checkboxes.
#[tauri::command]
fn toggle_task(path: String, index: usize, vault_root: Option<String>) -> Result<(), String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut seen = 0usize;
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut found = false;
    for line in lines.iter_mut() {
        let trimmed = line.trim_start();
        let is_task = (trimmed.starts_with("- [") || trimmed.starts_with("* [") || trimmed.starts_with("+ ["))
            && trimmed.as_bytes().get(4) == Some(&b']');
        if is_task {
            if seen == index {
                let state = trimmed.as_bytes()[3] as char;
                let new_state = if state == ' ' { 'x' } else { ' ' };
                let pos = line.len() - trimmed.len() + 3;
                line.replace_range(pos..pos + 1, &new_state.to_string());
                found = true;
                break;
            }
            seen += 1;
        }
    }
    if !found {
        return Err(format!("No task #{} in {}", index, path));
    }
    let mut out = lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    save_file(path, out, vault_root)
}

/// Create a note (optionally from a template / as today's daily note) and
/// return its absolute path.
#[tauri::command]
fn create_note(root: String, title: Option<String>, daily: bool) -> Result<String, String> {
    let v = vault_tools::create_note_from_template(Path::new(&root), title.as_deref(), None, daily)
        .map_err(|e| e.to_string())?;
    vault::invalidate(Path::new(&root));
    let rel = v["created"].as_str().ok_or("create returned no path")?;
    Ok(Path::new(&root).join(rel).to_string_lossy().into_owned())
}

/// Rename/move a note, rewriting every inbound wikilink (real run).
#[tauri::command]
fn rename_note(root: String, path: String, new_name: String) -> Result<String, String> {
    let rel = vault_rel(&root, &path).ok_or("File is outside the vault")?;
    let v = vault_tools::rename_note(Path::new(&root), &rel, &new_name, false)
        .map_err(|e| e.to_string())?;
    vault::invalidate(Path::new(&root));
    let new_rel = v["renamed_to"].as_str().unwrap_or(&new_name);
    Ok(Path::new(&root).join(new_rel).to_string_lossy().into_owned())
}

/// Replace the YAML frontmatter block of a note with the given YAML text
/// (empty string removes it), leaving the body untouched.
#[tauri::command]
fn set_frontmatter(path: String, yaml: String, vault_root: Option<String>) -> Result<(), String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let (_, body) = to_markdown_mcp::obsidian::frontmatter::split(&content);
    let yaml = yaml.trim();
    let new_content = if yaml.is_empty() {
        body.to_string()
    } else {
        // Validate before writing so a typo can't corrupt the note.
        serde_yaml::from_str::<serde_yaml::Value>(yaml).map_err(|e| format!("Invalid YAML: {}", e))?;
        format!("---\n{}\n---\n{}", yaml, body)
    };
    save_file(path, new_content, vault_root)
}

/// Save a pasted image into the vault's attachment folder and return the
/// wikilink embed text to insert.
#[tauri::command]
fn paste_image(root: String, base64_data: String, extension: String) -> Result<String, String> {
    let bytes = base64_decode(&base64_data).ok_or("Invalid base64 image data")?;
    let cfg = to_markdown_mcp::obsidian::config::read_config(Path::new(&root)).unwrap_or_default();
    let folder = cfg.attachment_folder.unwrap_or_default();
    let dir = if folder.is_empty() || folder == "/" {
        PathBuf::from(&root)
    } else {
        PathBuf::from(&root).join(folder.trim_start_matches("./"))
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = format!("Pasted image {}.{}", chrono_stamp(), extension);
    std::fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?;
    vault::invalidate(Path::new(&root));
    Ok(format!("![[{}]]", name))
}

fn chrono_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{}", secs)
}

fn base64_decode(data: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(data.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for &b in data.as_bytes() {
        if b == b'=' || b == b'\n' || b == b'\r' {
            continue;
        }
        let v = TABLE.iter().position(|&t| t == b)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// Tag names for the editor's `#` autocomplete.
#[tauri::command]
fn tag_list(root: String) -> Result<Vec<String>, String> {
    let idx = vault::get_index(Path::new(&root)).map_err(|e| e.to_string())?;
    let mut tags: Vec<String> = idx.tags.keys().cloned().collect();
    tags.sort();
    Ok(tags)
}

// ---- Intelligence (Phase 9) ----

/// Notes similar to the given one, ranked by TF-cosine over the vault.
#[tauri::command]
async fn related_notes(root: String, path: String) -> Result<serde_json::Value, String> {
    use to_markdown_mcp::knowledge;
    let idx = vault::get_index(Path::new(&root)).map_err(|e| e.to_string())?;
    let target_rel = vault_rel(&root, &path);
    let target = convert_any_to_markdown(Path::new(&path)).map_err(|e| e.to_string())?;
    let ttf = knowledge::term_frequencies(&target);
    let mut results: Vec<(String, f64)> = idx
        .notes
        .keys()
        .filter(|p| Some(p.as_str()) != target_rel.as_deref())
        .filter_map(|p| {
            let content = std::fs::read_to_string(Path::new(&root).join(p)).ok()?;
            let score = knowledge::cosine_similarity(&ttf, &knowledge::term_frequencies(&content));
            (score > 0.0).then_some((p.clone(), score))
        })
        .collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(8);
    Ok(serde_json::json!(results
        .into_iter()
        .map(|(p, s)| serde_json::json!({"path": p, "score": s}))
        .collect::<Vec<_>>()))
}

/// Vector-similarity search over the vault, using the persistent
/// `.tomarkdown` embedding index (hashed-vector fallback without a model).
#[tauri::command]
async fn semantic_search(root: String, query: String) -> Result<serde_json::Value, String> {
    use to_markdown_mcp::embeddings;
    let rootp = PathBuf::from(&root);
    let idx = vault::get_index(&rootp).map_err(|e| e.to_string())?;
    let sources: Vec<PathBuf> = idx.notes.keys().map(|p| rootp.join(p)).collect();
    let mut embedder = embeddings::default_embedder();
    let mut vindex = embeddings::VectorIndex::load(&rootp);
    vindex
        .update(&sources, embedder.as_mut(), |p| convert_any_to_markdown(p).ok())
        .map_err(|e| e.to_string())?;
    let _ = vindex.save(&rootp);
    let qv = embedder.embed(std::slice::from_ref(&query)).map_err(|e| e.to_string())?;
    let hits: Vec<serde_json::Value> = vindex
        .rank(&qv[0])
        .into_iter()
        .filter(|(_, _, s)| *s > 0.0)
        .take(12)
        .map(|(source, chunk, score)| {
            let snippet: String = chunk.text.chars().take(160).collect();
            serde_json::json!({
                "path": source,
                "heading": chunk.heading_path.join(" › "),
                "score": score,
                "snippet": snippet,
            })
        })
        .collect();
    Ok(serde_json::json!(hits))
}

/// Store/clear the Anthropic API key for this process (enables ai actions).
#[tauri::command]
fn set_api_key(key: String) {
    if key.trim().is_empty() {
        std::env::remove_var("ANTHROPIC_API_KEY");
    } else {
        std::env::set_var("ANTHROPIC_API_KEY", key.trim());
    }
}

/// Claude-backed actions on the open document (needs the API key).
#[tauri::command]
async fn ai_action(kind: String, content: String, extra: Option<String>) -> Result<String, String> {
    use to_markdown_mcp::llm;
    if llm::api_key().is_none() {
        return Err("No Anthropic API key. Add one in Settings (⌘,) to enable AI actions.".to_string());
    }
    let doc: String = content.chars().take(40_000).collect();
    let (system, prompt, max_tokens) = match kind.as_str() {
        "summarize" => (
            "You summarize documents faithfully and concisely in Markdown.",
            format!("Summarize the following document. Lead with a one-paragraph TL;DR, then key points as bullets.\n\n---\n{}", doc),
            700,
        ),
        "tag" => (
            "You suggest topical tags for notes.",
            format!("Suggest 3-8 topical tags for this document as a single line of #kebab-case tags, nothing else.\n\n---\n{}", doc),
            120,
        ),
        "translate" => (
            "You translate documents preserving all Markdown structure.",
            format!("Translate the following document to {}. Preserve Markdown exactly.\n\n---\n{}", extra.as_deref().unwrap_or("English"), doc),
            4000,
        ),
        "ask" => (
            "You answer questions grounded strictly in the provided document. Say so when the document doesn't contain the answer.",
            format!("Document:\n---\n{}\n---\n\nQuestion: {}", doc, extra.as_deref().unwrap_or("Summarize this.")),
            900,
        ),
        other => return Err(format!("Unknown AI action: {}", other)),
    };
    let model = llm::resolve_model(None);
    llm::complete(&prompt, Some(system), &model, max_tokens)
        .await
        .map_err(|e| e.to_string())
}

// ---- Phase B: document stats & hover peek ----

/// Marked 2-style document statistics: readability + structure + top words.
#[tauri::command]
fn doc_stats(content: String) -> serde_json::Value {
    use to_markdown_mcp::{doc_intel, rag};
    let r = doc_intel::readability(&content);
    let stats = rag::text_statistics(&content, true, 3);
    let top_words: Vec<serde_json::Value> = stats
        .frequencies
        .iter()
        .take(15)
        .map(|(w, c)| serde_json::json!({"word": w, "count": c}))
        .collect();
    serde_json::json!({
        "flesch_reading_ease": r.flesch_reading_ease,
        "flesch_kincaid_grade": r.flesch_kincaid_grade,
        "interpretation": doc_intel::flesch_interpretation(r.flesch_reading_ease),
        "words": r.words,
        "sentences": r.sentences,
        "avg_sentence_length": r.avg_sentence_length,
        "distinct_words": stats.distinct_words,
        "top_words": top_words,
    })
}

/// Rendered snippet of a wikilink target, for the hover page-preview.
/// Resolves relative to `from` (the note being read); broken links and
/// non-Markdown targets return an informational preview instead of an error
/// so every hover shows something.
#[tauri::command]
fn peek_note(root: String, target: String, from: Option<String>) -> Result<serde_json::Value, String> {
    let rootp = PathBuf::from(&root);
    let idx = vault::get_index(&rootp).map_err(|e| e.to_string())?;
    let from_rel = from.as_deref().and_then(|f| vault_rel(&root, f));
    let rel = match vault::resolve_target(&idx, &target, from_rel.as_deref()) {
        vault::Resolution::Resolved(r) => r,
        vault::Resolution::Ambiguous(mut c) => {
            c.sort_by_key(|p| p.len());
            match c.into_iter().next() {
                Some(r) => r,
                None => return Ok(peek_missing(&target)),
            }
        }
        vault::Resolution::Broken => return Ok(peek_missing(&target)),
    };
    let abs = rootp.join(&rel);
    let is_md = abs.extension().and_then(|e| e.to_str()).is_none_or(|e| e == "md");
    if !is_md {
        // Non-note target (canvas, attachment, ...): show basic file info.
        let size = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
        return Ok(serde_json::json!({
            "path": rel,
            "html": format!(
                "<em>{} — {:.1} KB. Click the link to open it.</em>",
                abs.extension().and_then(|e| e.to_str()).unwrap_or("file"),
                size as f64 / 1024.0
            ),
            "truncated": false,
        }));
    }
    let content = std::fs::read_to_string(&abs).map_err(|e| e.to_string())?;
    let (_, body) = to_markdown_mcp::obsidian::frontmatter::split(&content);
    let snippet: String = body.lines().take(30).collect::<Vec<_>>().join("\n");
    let opts = RenderOpts { file_dir: abs.parent(), vault_root: Some(&rootp) };
    Ok(serde_json::json!({
        "path": rel,
        "html": render_note(&snippet, &opts, 1),
        "truncated": body.lines().count() > 30,
    }))
}

fn peek_missing(target: &str) -> serde_json::Value {
    serde_json::json!({
        "path": "not found",
        "html": format!("<em>No note found for “{}”.</em>", target.replace('<', "&lt;")),
        "truncated": false,
    })
}

/// Read a small text file (user CSS). Capped at 1MB.
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    let size = std::fs::metadata(&p).map(|m| m.len()).map_err(|e| e.to_string())?;
    if size > 1024 * 1024 {
        return Err("File larger than 1MB".to_string());
    }
    std::fs::read_to_string(&p).map_err(|e| e.to_string())
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn pick_file(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(WatchState::default())
        .invoke_handler(tauri::generate_handler![
            list_tree, open_file, pick_folder, pick_file,
            watch_tree, watch_file, export_html, read_text_file,
            note_info, vault_overview, vault_search, vault_tasks,
            resolve_wikilink, graph_data, quick_list,
            read_source, save_file, render_markdown, toggle_task,
            create_note, rename_note, set_frontmatter, paste_image, tag_list,
            related_notes, semantic_search, set_api_key, ai_action, syntax_css,
            doc_stats, peek_note
        ])
        .run(tauri::generate_context!())
        .expect("error while running toMarkdown Viewer");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_vault() -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/mini_vault")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn wikilinks_become_clickable_anchors() {
        let root = PathBuf::from(fixture_vault());
        let opts = RenderOpts { file_dir: Some(&root), vault_root: Some(&root) };
        let html = render_note("See [[Note B|the second]] and [[Note A#Heading]].", &opts, 0);
        assert!(html.contains(r#"href="wikilink:Note%20B""#), "html: {}", html);
        assert!(html.contains(">the second<"));
        assert!(html.contains(r#"href="wikilink:Note%20A#Heading""#));
    }

    #[test]
    fn resolve_wikilink_finds_fixture_note() {
        let root = fixture_vault();
        let resolved = resolve_wikilink(root.clone(), "Note B".into(), format!("{}/Note A.md", root)).unwrap();
        assert!(resolved.ends_with("Note B.md"));
        assert!(resolve_wikilink(root, "No Such Note".into(), String::new()).is_err());
    }

    #[test]
    fn graph_data_links_fixture_notes() {
        let root = fixture_vault();
        let g = tauri::async_runtime::block_on(graph_data(root.clone(), None)).unwrap();
        assert!(!g["nodes"].as_array().unwrap().is_empty());
        assert!(!g["links"].as_array().unwrap().is_empty());
        // Local graph keeps only the focused note's neighborhood.
        let local = tauri::async_runtime::block_on(graph_data(root.clone(), Some(format!("{}/Note A.md", root)))).unwrap();
        assert!(local["nodes"].as_array().unwrap().len() <= g["nodes"].as_array().unwrap().len());
    }

    #[test]
    fn note_info_returns_properties_and_backlinks() {
        let root = fixture_vault();
        let info = note_info(root.clone(), format!("{}/Note A.md", root)).unwrap();
        assert_eq!(info["note"]["title"], "Note A");
        assert!(info["backlinks"]["backlink_count"].as_u64().is_some());
    }

    fn temp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("gui_edit_test_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn save_is_atomic_and_roundtrips() {
        let dir = temp_dir();
        let f = dir.join("note.md");
        save_file(f.to_string_lossy().into(), "# Hi\n".into(), None).unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "# Hi\n");
        assert!(!f.with_extension("tomarkdown.tmp").exists());
    }

    #[test]
    fn toggle_task_flips_the_right_checkbox() {
        let dir = temp_dir();
        let f = dir.join("tasks.md");
        std::fs::write(&f, "- [ ] first\ntext\n- [x] second\n  - [ ] nested\n").unwrap();
        let p = f.to_string_lossy().into_owned();
        toggle_task(p.clone(), 1, None).unwrap();
        assert!(std::fs::read_to_string(&f).unwrap().contains("- [ ] second"));
        toggle_task(p.clone(), 2, None).unwrap();
        assert!(std::fs::read_to_string(&f).unwrap().contains("  - [x] nested"));
        assert!(toggle_task(p, 9, None).is_err());
    }

    #[test]
    fn set_frontmatter_replaces_only_the_yaml_block() {
        let dir = temp_dir();
        let f = dir.join("fm.md");
        std::fs::write(&f, "---\nstatus: old\n---\n# Body\n").unwrap();
        let p = f.to_string_lossy().into_owned();
        set_frontmatter(p.clone(), "status: new\ntags: [a]".into(), None).unwrap();
        let s = std::fs::read_to_string(&f).unwrap();
        assert!(s.contains("status: new") && s.contains("# Body"));
        // Invalid YAML is rejected, file untouched.
        assert!(set_frontmatter(p.clone(), "not: [valid".into(), None).is_err());
        // Empty removes the block.
        set_frontmatter(p, String::new(), None).unwrap();
        assert!(std::fs::read_to_string(&f).unwrap().starts_with("# Body"));
    }

    #[test]
    fn base64_decodes() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert!(base64_decode("!!!").is_none());
    }

    #[test]
    fn canvas_files_render() {
        let root = fixture_vault();
        let r = tauri::async_runtime::block_on(open_file(format!("{}/Board.canvas", root), Some(root))).unwrap();
        assert!(!r.html.trim().is_empty());
    }
}
