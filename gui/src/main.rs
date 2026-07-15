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

#[tauri::command]
fn open_file(path: String) -> Result<Rendered, String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("Not a file: {}", path));
    }
    let converted = convert_any_to_markdown(&p).map_err(|e| e.to_string())?;
    let words = converted.split_whitespace().count();
    let chars = converted.chars().count();
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
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
            watch_tree, watch_file, export_html, read_text_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running toMarkdown Viewer");
}
