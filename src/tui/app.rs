use std::path::PathBuf;
use std::time::SystemTime;

/// Which pane owns keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Tree,
    Content,
}

/// Which pane a `/` search applies to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchTarget {
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
    pub search_target: SearchTarget,
    /// Line numbers (0-indexed) in the open note matching the last content search.
    pub content_matches: Vec<usize>,
    /// Cursor position when a content search began, restored on cancel.
    pub search_origin_cursor: usize,
    pub status: String,
    pub quit: bool,
    /// Content pane inner height, refreshed each draw; used for page/half-page jumps.
    pub view_height: usize,
    /// mtime of the currently open file, for live-reload.
    pub current_mtime: Option<SystemTime>,
    pub current_tags: Vec<String>,
    pub current_backlink_count: usize,
    /// View the open note as raw source instead of styled Markdown. Persists
    /// across notes until toggled.
    pub raw_view: bool,
    pub show_help: bool,
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
            search_target: SearchTarget::Tree,
            content_matches: Vec::new(),
            search_origin_cursor: 0,
            status: "? for help · q quit".into(),
            quit: false,
            view_height: 20,
            current_mtime: None,
            current_tags: Vec::new(),
            current_backlink_count: 0,
            raw_view: false,
            show_help: false,
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

    /// Enter search-input mode. Targets the file tree or the open note's
    /// content depending on which pane currently has focus.
    pub fn begin_search(&mut self) {
        self.searching = true;
        self.search.clear();
        self.search_target = match self.focus {
            Focus::Tree => SearchTarget::Tree,
            Focus::Content => SearchTarget::Content,
        };
        if self.search_target == SearchTarget::Content {
            self.search_origin_cursor = self.cursor;
            self.content_matches.clear();
        }
    }

    pub fn type_search_char(&mut self, c: char) {
        self.search.push(c);
        self.recompute_search();
    }

    pub fn backspace_search(&mut self) {
        self.search.pop();
        self.recompute_search();
    }

    fn recompute_search(&mut self) {
        match self.search_target {
            SearchTarget::Tree => self.apply_filter(),
            SearchTarget::Content => self.recompute_content_matches(),
        }
    }

    fn recompute_content_matches(&mut self) {
        let q = self.search.to_lowercase();
        if q.is_empty() {
            self.content_matches.clear();
            return;
        }
        self.content_matches = self
            .content
            .lines()
            .enumerate()
            .filter(|(_, line)| line.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        if let Some(&next) = self
            .content_matches
            .iter()
            .find(|&&i| i >= self.search_origin_cursor)
            .or_else(|| self.content_matches.first())
        {
            self.cursor = next;
        }
    }

    /// Commit the in-progress search: exits input mode but keeps
    /// `content_matches` (if any) so `n`/`N` keep working afterward.
    pub fn commit_search(&mut self) {
        self.searching = false;
    }

    /// Abort the in-progress search: exits input mode, restores the cursor
    /// to where the content search began, and clears matches entirely.
    pub fn cancel_search(&mut self) {
        self.searching = false;
        if self.search_target == SearchTarget::Content {
            self.cursor = self.search_origin_cursor;
            self.content_matches.clear();
        }
        self.search.clear();
    }

    /// Jump to the next content-search match after the cursor, wrapping.
    pub fn next_match(&mut self) {
        if self.content_matches.is_empty() {
            self.status = "No active search (press / in the content pane)".into();
            return;
        }
        let next = self
            .content_matches
            .iter()
            .find(|&&i| i > self.cursor)
            .or_else(|| self.content_matches.first())
            .copied();
        if let Some(line) = next {
            self.cursor = line;
            self.report_match_position();
        }
    }

    /// Jump to the previous content-search match before the cursor, wrapping.
    pub fn prev_match(&mut self) {
        if self.content_matches.is_empty() {
            self.status = "No active search (press / in the content pane)".into();
            return;
        }
        let prev = self
            .content_matches
            .iter()
            .rev()
            .find(|&&i| i < self.cursor)
            .or_else(|| self.content_matches.last())
            .copied();
        if let Some(line) = prev {
            self.cursor = line;
            self.report_match_position();
        }
    }

    fn report_match_position(&mut self) {
        if let Some(pos) = self.content_matches.iter().position(|&i| i == self.cursor) {
            self.status = format!("match {}/{}", pos + 1, self.content_matches.len());
        }
    }

    pub fn toggle_raw_view(&mut self) {
        self.raw_view = !self.raw_view;
        self.status = if self.raw_view { "raw view".into() } else { "formatted view".into() };
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
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
                self.content_matches.clear();
                self.focus = Focus::Content;
                self.current_mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                self.refresh_vault_info();
                self.status = format!("{} · ? for help", rel);
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

    fn app_with_content(content: &str) -> App {
        let dir = scratch_dir("search");
        std::fs::write(dir.join("A.md"), content).unwrap();
        let mut app = App::new(dir, vec!["A.md".into()]);
        app.open("A.md", false);
        app
    }

    #[test]
    fn content_search_jumps_and_wraps() {
        let mut app = app_with_content("alpha\nbeta\nalpha again\nbeta again\nalpha third");
        app.cursor = 0;
        app.begin_search();
        assert_eq!(app.search_target, SearchTarget::Content);
        app.type_search_char('a');
        app.type_search_char('l');
        app.type_search_char('p');
        app.type_search_char('h');
        app.type_search_char('a');
        // Live-jump lands on the match at/after the origin cursor (line 0 itself).
        assert_eq!(app.cursor, 0);
        app.commit_search();
        assert!(!app.searching);
        assert_eq!(app.content_matches, vec![0, 2, 4]);

        app.next_match();
        assert_eq!(app.cursor, 2);
        app.next_match();
        assert_eq!(app.cursor, 4);
        app.next_match(); // wraps
        assert_eq!(app.cursor, 0);

        app.prev_match(); // wraps backward
        assert_eq!(app.cursor, 4);
        app.prev_match();
        assert_eq!(app.cursor, 2);
    }

    #[test]
    fn cancel_search_restores_cursor_and_clears_matches() {
        let mut app = app_with_content("one\ntwo\nthree\ntwo again");
        app.cursor = 0;
        app.begin_search();
        app.type_search_char('t');
        app.type_search_char('w');
        app.type_search_char('o');
        assert_ne!(app.content_matches.len(), 0);

        app.cancel_search();
        assert!(!app.searching);
        assert_eq!(app.cursor, 0); // restored to origin
        assert!(app.content_matches.is_empty());
        assert!(app.search.is_empty());

        // n/N report "no active search" rather than panicking or moving.
        app.next_match();
        assert_eq!(app.cursor, 0);
        assert!(app.status.contains("No active search"));
    }

    #[test]
    fn tree_search_unaffected_by_content_search_logic() {
        let dir = scratch_dir("tree_search");
        let mut app = App::new(dir.clone(), vec!["Alpha.md".into(), "Beta.md".into()]);
        app.focus = Focus::Tree;
        app.begin_search();
        assert_eq!(app.search_target, SearchTarget::Tree);
        app.type_search_char('b');
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.selected_file(), Some(&"Beta.md".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn toggle_raw_view_persists_across_open() {
        let dir = scratch_dir("raw_persist");
        std::fs::write(dir.join("A.md"), "one").unwrap();
        std::fs::write(dir.join("B.md"), "two").unwrap();
        let mut app = App::new(dir.clone(), vec!["A.md".into(), "B.md".into()]);
        app.open("A.md", false);
        assert!(!app.raw_view);
        app.toggle_raw_view();
        assert!(app.raw_view);

        app.open("B.md", true);
        assert!(app.raw_view, "raw_view should persist across notes");

        app.toggle_raw_view();
        assert!(!app.raw_view);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn toggle_help_flips_flag() {
        let dir = scratch_dir("help");
        let mut app = App::new(dir.clone(), vec![]);
        assert!(!app.show_help);
        app.toggle_help();
        assert!(app.show_help);
        app.toggle_help();
        assert!(!app.show_help);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
