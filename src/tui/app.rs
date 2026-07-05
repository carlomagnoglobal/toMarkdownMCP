use std::path::PathBuf;

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
            status: "q quit · Enter open/follow · Tab switch pane · / search · Backspace back".into(),
            quit: false,
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
        match std::fs::read_to_string(self.root.join(rel)) {
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
                self.status = format!("{} · q quit · Backspace back", rel);
            }
            Err(e) => self.status = format!("Cannot open {}: {}", rel, e),
        }
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
