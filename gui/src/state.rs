use std::collections::{HashMap, VecDeque};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: String,
    pub path: String,
    pub title: String,
    pub is_dirty: bool,
    pub tab_type: String, // "markdown", "code", "image", "hex"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedFile {
    pub id: String,
    pub original_path: String,
    pub vault_path: String,
    pub deleted_at: i64, // Unix timestamp
    pub file_size: u64,
}

#[derive(Debug, Clone)]
pub struct VaultViewerState {
    pub open_tabs: Vec<Tab>,
    pub active_tab: Option<String>,
    pub preview_tab: Option<String>,
    pub tab_history: VecDeque<String>, // tab IDs for back button
    pub deleted_files: Vec<DeletedFile>,
    pub zoom_levels: HashMap<String, f32>, // file_id -> zoom level
    pub user_preferences: UserPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub tab_mode: TabMode,
    pub theme: String,
    pub auto_restore_tabs: bool,
    pub recycle_retention_days: u32,
    pub auto_save: bool,
    pub show_toast: bool,
    pub zoom_behavior: ZoomBehavior,
    pub mouse_wheel_zoom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TabMode {
    Single,
    Multi,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ZoomBehavior {
    ResetPerImage,
    RememberPerImage,
    RememberGlobal,
}

impl VaultViewerState {
    pub fn new(preferences: UserPreferences) -> Self {
        VaultViewerState {
            open_tabs: Vec::new(),
            active_tab: None,
            preview_tab: None,
            tab_history: VecDeque::new(),
            deleted_files: Vec::new(),
            zoom_levels: HashMap::new(),
            user_preferences: preferences,
        }
    }

    pub fn add_tab(&mut self, path: String, title: String, tab_type: String) -> String {
        let id = Uuid::new_v4().to_string();
        let tab = Tab {
            id: id.clone(),
            path,
            title,
            is_dirty: false,
            tab_type,
        };
        self.open_tabs.push(tab);
        self.set_active_tab(id.clone());
        id
    }

    pub fn set_active_tab(&mut self, tab_id: String) {
        if let Some(current) = &self.active_tab {
            if current != &tab_id {
                self.tab_history.push_back(current.clone());
                if self.tab_history.len() > 50 {
                    self.tab_history.pop_front();
                }
            }
        }
        self.active_tab = Some(tab_id);
    }

    pub fn close_tab(&mut self, tab_id: &str) {
        self.open_tabs.retain(|t| t.id != tab_id);
        if self.active_tab.as_ref().map_or(false, |id| id == tab_id) {
            self.active_tab = self.open_tabs.first().map(|t| t.id.clone());
        }
    }

    pub fn back(&mut self) {
        if let Some(tab_id) = self.tab_history.pop_back() {
            self.active_tab = Some(tab_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_state() {
        let prefs = UserPreferences {
            tab_mode: TabMode::Multi,
            theme: "light".to_string(),
            auto_restore_tabs: true,
            recycle_retention_days: 180,
            auto_save: true,
            show_toast: true,
            zoom_behavior: ZoomBehavior::ResetPerImage,
            mouse_wheel_zoom: true,
        };
        let state = VaultViewerState::new(prefs);
        assert_eq!(state.open_tabs.len(), 0);
        assert_eq!(state.active_tab, None);
    }

    #[test]
    fn test_add_tab() {
        let mut state = VaultViewerState::new(default_preferences());
        let id = state.add_tab(
            "/path/to/file.md".to_string(),
            "file.md".to_string(),
            "markdown".to_string(),
        );
        assert_eq!(state.open_tabs.len(), 1);
        assert_eq!(state.active_tab, Some(id));
    }

    #[test]
    fn test_close_tab() {
        let mut state = VaultViewerState::new(default_preferences());
        let id1 = state.add_tab(
            "/path/to/file1.md".to_string(),
            "file1.md".to_string(),
            "markdown".to_string(),
        );
        let _id2 = state.add_tab(
            "/path/to/file2.md".to_string(),
            "file2.md".to_string(),
            "markdown".to_string(),
        );
        state.close_tab(&id1);
        assert_eq!(state.open_tabs.len(), 1);
    }

    #[test]
    fn test_back_button() {
        let mut state = VaultViewerState::new(default_preferences());
        let id1 = state.add_tab(
            "/path/to/file1.md".to_string(),
            "file1.md".to_string(),
            "markdown".to_string(),
        );
        let _id2 = state.add_tab(
            "/path/to/file2.md".to_string(),
            "file2.md".to_string(),
            "markdown".to_string(),
        );
        state.back();
        assert_eq!(state.active_tab, Some(id1));
    }

    fn default_preferences() -> UserPreferences {
        UserPreferences {
            tab_mode: TabMode::Multi,
            theme: "light".to_string(),
            auto_restore_tabs: true,
            recycle_retention_days: 180,
            auto_save: true,
            show_toast: true,
            zoom_behavior: ZoomBehavior::ResetPerImage,
            mouse_wheel_zoom: true,
        }
    }
}
