use std::path::PathBuf;
use std::time::SystemTime;

/// Which pane owns keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Tree,
    Content,
}

pub struct App {
    pub root: PathBuf,
    /// Vault-relative note paths (sorted).
    pub files: Vec<String>,
    /// Filtered view of `files` when searching.
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub focus: Focus,
    /// Currently open note (vault-relative) and its raw content.
    pub current: Option<String>,
    pub content: String,
    pub scroll: u16,
    /// Cursor line within the content (0-indexed), for link following.
    pub cursor: usize,
    pub history: Vec<String>,
    pub searching: bool,
    pub search: String,
    pub status: String,
    pub quit: bool,
    /// Content pane inner height, refreshed each draw; used for page/half-page jumps.
    pub view_height: usize,
    /// mtime of the currently open file, for live-reload.
    pub current_mtime: Option<SystemTime>,
    pub current_tags: Vec<String>,
    pub current_backlink_count: usize,
}

impl App {
    pub fn new(root: PathBuf, files: Vec<String>) -> Self {
        let filtered = (0..files.len()).collect();
        App {
            root,
            files,
            filtered,
            selected: 0,
            focus: Focus::Tree,
            current: None,
            content: String::new(),
            scroll: 0,
            cursor: 0,
            history: Vec::new(),
            searching: false,
            search: String::new(),
            status: "q quit · Enter open/follow · Tab pane · / search · Backspace back · g/G top/bottom · ^f/^b/^d/^u page".into(),
            quit: false,
            view_height: 20,
            current_mtime: None,
            current_tags: Vec::new(),
            current_backlink_count: 0,
        }
    }

    pub fn apply_filter(&mut self) {
        let q = self.search.to_lowercase();
        self.filtered = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| q.is_empty() || f.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn selected_file(&self) -> Option<&String> {
        self.filtered.get(self.selected).map(|&i| &self.files[i])
    }

    pub fn open(&mut self, rel: &str, push_history: bool) {
        let path = self.root.join(rel);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                if push_history {
                    if let Some(cur) = self.current.take() {
                        self.history.push(cur);
                    }
                }
                self.current = Some(rel.to_string());
                self.content = content;
                self.scroll = 0;
                self.cursor = 0;
                self.focus = Focus::Content;
                self.current_mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                self.refresh_vault_info();
                self.status = format!("{} · q quit · Backspace back", rel);
            }
            Err(e) => self.status = format!("Cannot open {}: {}", rel, e),
        }
    }

    /// Recompute tags/backlink count for the current note from the vault index.
    fn refresh_vault_info(&mut self) {
        self.current_tags.clear();
        self.current_backlink_count = 0;
        let Some(rel) = self.current.clone() else { return };
        let Ok(idx) = crate::obsidian::vault::get_index(&self.root) else { return };
        if let Some(meta) = idx.notes.get(&rel) {
            self.current_tags = meta.tags.clone();
        }
        self.current_backlink_count = idx.backlinks.get(&rel).map(|b| b.len()).unwrap_or(0);
    }

    /// Reload the current file if it changed on disk since it was opened.
    /// Called on every tick; a single `stat` call keeps this cheap.
    pub fn maybe_reload(&mut self) {
        let Some(rel) = self.current.clone() else { return };
        let path = self.root.join(&rel);
        let Ok(meta) = std::fs::metadata(&path) else { return };
        let Ok(mtime) = meta.modified() else { return };
        if Some(mtime) == self.current_mtime {
            return;
        }
        let Ok(content) = std::fs::read_to_string(&path) else { return };
        self.content = content;
        self.current_mtime = Some(mtime);
        let max = self.content.lines().count().saturating_sub(1);
        if self.cursor > max {
            self.cursor = max;
        }
        self.refresh_vault_info();
        self.status = format!("{} · reloaded · q quit · Backspace back", rel);
    }

    pub fn back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.current = None; // avoid re-pushing
            self.open(&prev, false);
        } else {
            self.focus = Focus::Tree;
        }
    }

    /// Follow the first wikilink on the cursor line, if any.
    pub fn follow_link(&mut self) {
        let Some(line) = self.content.lines().nth(self.cursor) else { return };
        let targets = super::render::wikilinks_in_line(line);
        let Some(target) = targets.first() else {
            self.status = "No [[wikilink]] on this line".into();
            return;
        };
        let from = self.current.clone();
        match crate::obsidian::vault::get_index(&self.root) {
            Ok(idx) => match crate::obsidian::vault::resolve_target(&idx, target, from.as_deref()) {
                crate::obsidian::vault::Resolution::Resolved(path) => {
                    if path.ends_with(".md") || path.ends_with(".markdown") {
                        self.open(&path, true);
                    } else {
                        self.status = format!("{} is an attachment", path);
                    }
                }
                crate::obsidian::vault::Resolution::Ambiguous(c) => {
                    self.status = format!("Ambiguous: {}", c.join(", "));
                }
                crate::obsidian::vault::Resolution::Broken => {
                    self.status = format!("Broken link: {}", target);
                }
            },
            Err(e) => self.status = format!("Index error: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tui_app_test_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn open_tracks_mtime_and_vault_info() {
        let dir = scratch_dir("open");
        std::fs::write(dir.join("A.md"), "---\ntags: [x]\n---\nlink to [[B]]").unwrap();
        std::fs::write(dir.join("B.md"), "back to [[A]]").unwrap();

        let mut app = App::new(dir.clone(), vec!["A.md".into(), "B.md".into()]);
        app.open("A.md", false);
        assert!(app.current_mtime.is_some());
        assert_eq!(app.current_tags, vec!["x".to_string()]);
        assert_eq!(app.current_backlink_count, 1); // linked from B.md

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn maybe_reload_picks_up_disk_changes() {
        let dir = scratch_dir("reload");
        let path = dir.join("A.md");
        std::fs::write(&path, "line one\nline two").unwrap();

        let mut app = App::new(dir.clone(), vec!["A.md".into()]);
        app.open("A.md", false);
        app.cursor = 1;
        let before = app.current_mtime;

        // No change yet: no-op.
        app.maybe_reload();
        assert_eq!(app.content, "line one\nline two");

        // Ensure a distinct mtime on filesystems with coarse timestamp resolution.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut f = std::fs::OpenOptions::new().write(true).truncate(true).open(&path).unwrap();
        f.write_all(b"only one line now").unwrap();
        drop(f);

        app.maybe_reload();
        assert_eq!(app.content, "only one line now");
        assert_ne!(app.current_mtime, before);
        assert_eq!(app.cursor, 0); // clamped: file shrank to one line

        let _ = std::fs::remove_dir_all(&dir);
    }
}
