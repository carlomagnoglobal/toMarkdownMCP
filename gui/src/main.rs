//! Desktop viewer for toMarkdownMCP: file tree + rendered Markdown pane.
//! All conversion/vault logic comes from the `to_markdown_mcp` library; this
//! crate only adds Tauri commands and Markdown→HTML rendering for the webview.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use notify::Watcher;
use pulldown_cmark::{html, Options, Parser};
use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

use to_markdown_mcp::file_type::{detect_language, detect_language_from_filename};
use to_markdown_mcp::obsidian::{tools as vault_tools, vault, wikilink};
use to_markdown_mcp::pipeline::convert_any_to_markdown;

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
fn list_tree(root: String) -> Result<Vec<TreeNode>, String> {
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

fn markdown_to_html(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    let mut out = String::with_capacity(md.len() * 2);
    html::push_html(&mut out, Parser::new_ext(md, options));
    out
}

/// Replace `[[wikilinks]]` with `wikilink:` markdown links so the rendered
/// HTML gets clickable anchors the frontend resolves against the vault.
fn linkify_wikilinks(md: &str) -> String {
    let mut out = md.to_string();
    for link in wikilink::parse_wikilinks(md) {
        let mut label = link.alias.clone().unwrap_or_else(|| link.target.clone());
        if link.alias.is_none() {
            if let Some(h) = &link.heading {
                label = format!("{} › {}", label, h);
            }
        }
        if link.embed {
            label = format!("⧉ {}", label);
        }
        // Percent-encode so spaces don't terminate the markdown URL.
        let href = format!(
            "wikilink:{}{}",
            link.target.replace('%', "%25").replace(' ', "%20"),
            link.heading.as_ref().map(|h| format!("#{}", h.replace('%', "%25").replace(' ', "%20"))).unwrap_or_default(),
        );
        out = out.replace(&link.raw, &format!("[{}]({})", label, href));
    }
    out
}

#[tauri::command]
fn open_file(path: String, vault_root: Option<String>) -> Result<Rendered, String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("Not a file: {}", path));
    }
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    // Obsidian canvas files render via the JsonCanvas→Markdown converter.
    if ext == "canvas" {
        let value = vault_tools::convert_canvas(&p).map_err(|e| e.to_string())?;
        let md = value["markdown"].as_str().unwrap_or_default().to_string();
        let words = md.split_whitespace().count();
        return Ok(Rendered {
            title: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path),
            html: markdown_to_html(&linkify_wikilinks(&md)),
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
    let md = if matches!(ext, "md" | "markdown") {
        // Inside a vault, wikilinks become clickable anchors.
        if vault_root.is_some() { linkify_wikilinks(&converted) } else { converted }
    } else if to_markdown_mcp::pipeline::is_structured_ext(Some(ext)) {
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
        html: markdown_to_html(&md),
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
fn export_html(
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
fn vault_overview(root: String) -> Result<serde_json::Value, String> {
    vault_tools::vault_index(Path::new(&root), true).map_err(|e| e.to_string())
}

#[tauri::command]
fn vault_search(root: String, query: String, mode: String) -> Result<serde_json::Value, String> {
    vault_tools::search(Path::new(&root), &query, &mode, 50).map_err(|e| e.to_string())
}

#[tauri::command]
fn vault_tasks(root: String) -> Result<serde_json::Value, String> {
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
fn graph_data(root: String, focus: Option<String>) -> Result<serde_json::Value, String> {
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
fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn pick_file(app: tauri::AppHandle) -> Option<String> {
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
            resolve_wikilink, graph_data, quick_list
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
        let html = markdown_to_html(&linkify_wikilinks("See [[Note B|the second]] and [[Note A#Heading]]."));
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
        let g = graph_data(root.clone(), None).unwrap();
        assert!(!g["nodes"].as_array().unwrap().is_empty());
        assert!(!g["links"].as_array().unwrap().is_empty());
        // Local graph keeps only the focused note's neighborhood.
        let local = graph_data(root.clone(), Some(format!("{}/Note A.md", root))).unwrap();
        assert!(local["nodes"].as_array().unwrap().len() <= g["nodes"].as_array().unwrap().len());
    }

    #[test]
    fn note_info_returns_properties_and_backlinks() {
        let root = fixture_vault();
        let info = note_info(root.clone(), format!("{}/Note A.md", root)).unwrap();
        assert_eq!(info["note"]["title"], "Note A");
        assert!(info["backlinks"]["backlink_count"].as_u64().is_some());
    }

    #[test]
    fn canvas_files_render() {
        let root = fixture_vault();
        let r = open_file(format!("{}/Board.canvas", root), Some(root)).unwrap();
        assert!(!r.html.trim().is_empty());
    }
}
