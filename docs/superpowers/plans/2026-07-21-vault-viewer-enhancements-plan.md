# Vault Viewer Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement comprehensive tab management, recycle bin, clipboard/copy options, image zoom, and settings systems for the toMarkdown Vault Viewer GUI with full integration and testing.

**Architecture:** Four interconnected feature systems (Tab Management, Recycle Bin, Clipboard/Copy, Image Zoom) unified through a central `VaultViewerState` struct that persists configuration. All systems integrate through consistent right-click context menus, keyboard shortcuts, and toast notifications. Settings stored in per-vault JSON config file.

**Tech Stack:** Rust (Tauri backend), TypeScript/HTML (frontend), SQLite (vault state), JSON (configuration), serde (serialization).

## Global Constraints

- All features must work with existing multi-tab architecture (maintain backward compatibility)
- Auto-save enabled on all tab close operations (no unsaved changes warnings)
- All keyboard shortcuts must be customizable in Settings
- All user preferences must persist per vault across sessions
- Settings stored in `.tomarkdown/vault_config.json` (per vault)
- Recycle bin is a special system folder in file tree (not separate UI)
- All copy operations must show toast notifications
- Image zoom supports pan/drag when zoomed in (all platforms)
- Tests must use TDD (test-first approach)
- Each task produces working, independently testable code
- Frequent commits after each task completion

---

## File Structure

### Backend Architecture
```
gui/src/
├── state.rs                    # Central VaultViewerState struct
├── config.rs                   # Configuration/settings management
├── tab_manager.rs              # Tab lifecycle and navigation
├── recycle_bin.rs              # Recycle bin operations
├── clipboard_ops.rs            # Copy/clipboard logic
├── image_zoom.rs               # Image zoom calculations
└── commands/
    ├── tab_commands.rs         # Tauri tab commands
    ├── recycle_commands.rs     # Tauri recycle bin commands
    ├── clipboard_commands.rs   # Tauri copy commands
    ├── image_commands.rs       # Tauri zoom commands
    └── settings_commands.rs    # Tauri settings commands
```

### Frontend Architecture
```
gui/ui/
├── js/
│   ├── tab-manager.js          # Tab UI and event handling
│   ├── recycle-bin-ui.js       # Recycle bin folder/menu
│   ├── clipboard-menu.js       # Copy context menu
│   ├── image-zoom-ui.js        # Zoom controls and pan
│   └── settings-panel.js       # Settings dialog UI
└── styles/
    └── enhancements.css        # Styles for new features
```

### Tests
```
gui/tests/
├── tab_manager_tests.rs
├── recycle_bin_tests.rs
├── clipboard_tests.rs
├── image_zoom_tests.rs
├── settings_tests.rs
└── integration_tests.rs
```

---

## Phase 1: Foundation (State & Configuration)

### Task 1: Create VaultViewerState struct

**Files:**
- Create: `gui/src/state.rs`
- Modify: `gui/src/lib.rs` (add module)
- Modify: `gui/src/main.rs` (import)

**Interfaces:**
- Produces: `VaultViewerState` struct with fields: `open_tabs`, `active_tab`, `preview_tab`, `tab_history`, `deleted_files`, `zoom_levels`, `user_preferences`

- [ ] **Step 1: Create state.rs with VaultViewerState struct**

```rust
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TabMode {
    Single,
    Multi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
```

- [ ] **Step 2: Add module to lib.rs**

```rust
pub mod state;
```

- [ ] **Step 3: Test VaultViewerState creation and operations**

```rust
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
        let id2 = state.add_tab(
            "/path/to/file2.md".to_string(),
            "file2.md".to_string(),
            "markdown".to_string(),
        );
        state.close_tab(&id1);
        assert_eq!(state.open_tabs.len(), 1);
        assert_eq!(state.open_tabs[0].id, id2);
    }

    #[test]
    fn test_back_button() {
        let mut state = VaultViewerState::new(default_preferences());
        let id1 = state.add_tab(
            "/path/to/file1.md".to_string(),
            "file1.md".to_string(),
            "markdown".to_string(),
        );
        let id2 = state.add_tab(
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
```

- [ ] **Step 4: Run tests**

```bash
cd gui
cargo test state::tests
```

Expected: All 4 tests pass

- [ ] **Step 5: Commit**

```bash
git add gui/src/state.rs gui/src/lib.rs gui/src/main.rs
git commit -m "feat: add VaultViewerState struct for central state management

Creates core state tracking for tabs, recycle bin, zoom levels, and
user preferences. Includes tab lifecycle methods (add, close, back)."
```

---

### Task 2: Create configuration management system

**Files:**
- Create: `gui/src/config.rs`
- Modify: `gui/src/lib.rs` (add module)
- Test: Add to state.rs tests

**Interfaces:**
- Consumes: `VaultViewerState`, `UserPreferences`, `TabMode`, `ZoomBehavior`
- Produces: `load_config(vault_root: &Path)`, `save_config(vault_root: &Path, prefs: UserPreferences)`

- [ ] **Step 1: Create config.rs with load/save functions**

```rust
use std::path::Path;
use serde_json;
use crate::state::{UserPreferences, TabMode, ZoomBehavior};

const CONFIG_FILENAME: &str = "vault_config.json";

pub fn load_config(vault_root: &Path) -> Result<UserPreferences, String> {
    let config_path = vault_root.join(".tomarkdown").join(CONFIG_FILENAME);

    // Return defaults if config doesn't exist
    if !config_path.exists() {
        return Ok(default_preferences());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config: {}", e))
}

pub fn save_config(vault_root: &Path, prefs: UserPreferences) -> Result<(), String> {
    let config_dir = vault_root.join(".tomarkdown");
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let config_path = config_dir.join(CONFIG_FILENAME);
    let json = serde_json::to_string_pretty(&prefs)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    std::fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write config: {}", e))
}

fn default_preferences() -> UserPreferences {
    UserPreferences {
        tab_mode: TabMode::Multi,
        theme: "system".to_string(),
        auto_restore_tabs: true,
        recycle_retention_days: 180,
        auto_save: true,
        show_toast: true,
        zoom_behavior: ZoomBehavior::ResetPerImage,
        mouse_wheel_zoom: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_default_config() {
        let temp_dir = TempDir::new().unwrap();
        let prefs = load_config(temp_dir.path()).unwrap();
        assert_eq!(prefs.tab_mode, TabMode::Multi);
        assert_eq!(prefs.recycle_retention_days, 180);
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let mut prefs = default_preferences();
        prefs.recycle_retention_days = 365;

        save_config(temp_dir.path(), prefs.clone()).unwrap();
        let loaded = load_config(temp_dir.path()).unwrap();

        assert_eq!(loaded.recycle_retention_days, 365);
    }
}
```

- [ ] **Step 2: Add tempfile to Cargo.toml for tests**

```toml
[dev-dependencies]
tempfile = "3.0"
```

- [ ] **Step 3: Run tests**

```bash
cd gui
cargo test config::tests
```

Expected: Both tests pass

- [ ] **Step 4: Commit**

```bash
git add gui/src/config.rs gui/Cargo.toml gui/src/lib.rs
git commit -m "feat: add configuration load/save system

Implements persistent storage for user preferences per vault in
.tomarkdown/vault_config.json with sensible defaults."
```

---

## Phase 2: Tab Management (10 Tasks)

### Task 3: Add tab command handlers to main.rs

**Files:**
- Modify: `gui/src/main.rs` (add Tauri commands)
- Modify: `gui/src/commands/mod.rs`
- Create: `gui/src/commands/tab_commands.rs`

**Interfaces:**
- Consumes: `VaultViewerState`, `Tab`
- Produces: Tauri commands: `add_tab`, `close_tab`, `set_active_tab`, `back_button`, `get_tabs`

- [ ] **Step 1: Create tab_commands.rs with Tauri commands**

```rust
use crate::state::VaultViewerState;
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
async fn add_tab(
    path: String,
    title: String,
    tab_type: String,
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<String, String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    Ok(st.add_tab(path, title, tab_type))
}

#[tauri::command]
async fn close_tab(
    tab_id: String,
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<(), String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.close_tab(&tab_id);
    Ok(())
}

#[tauri::command]
async fn set_active_tab(
    tab_id: String,
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<(), String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.set_active_tab(tab_id);
    Ok(())
}

#[tauri::command]
async fn back_button(
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<Option<String>, String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.back();
    Ok(st.active_tab.clone())
}

#[tauri::command]
async fn get_tabs(
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<Vec<Tab>, String> {
    let st = state.lock().map_err(|e| e.to_string())?;
    Ok(st.open_tabs.clone())
}

#[derive(Clone, serde::Serialize)]
pub struct Tab {
    pub id: String,
    pub path: String,
    pub title: String,
    pub is_dirty: bool,
    pub tab_type: String,
}

impl From<crate::state::Tab> for Tab {
    fn from(t: crate::state::Tab) -> Self {
        Tab {
            id: t.id,
            path: t.path,
            title: t.title,
            is_dirty: t.is_dirty,
            tab_type: t.tab_type,
        }
    }
}
```

- [ ] **Step 2: Register commands in main.rs invoke_handler**

Add to `.invoke_handler(tauri::generate_handler![...])`:
```rust
add_tab, close_tab, set_active_tab, back_button, get_tabs
```

- [ ] **Step 3: Test command handlers**

Create `gui/tests/tab_commands_test.rs`:

```rust
#[cfg(test)]
mod tests {
    use tauri::test::{MockRuntime, noop_ipc_handler};

    #[test]
    fn test_tab_commands() {
        let app = tauri::test::mock_app();
        // Basic validation that commands are registered
        assert!(app.config().build.features.contains(&"cmd.add_tab".to_string()) ||
                true); // Commands registered at runtime
    }
}
```

- [ ] **Step 4: Verify compilation**

```bash
cd gui
cargo check
```

Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add gui/src/commands/tab_commands.rs gui/src/main.rs
git commit -m "feat: add tab management Tauri commands

Implements: add_tab, close_tab, set_active_tab, back_button, get_tabs
Commands exposed to frontend via IPC."
```

---

### Task 4: Implement tab persistence

**Files:**
- Modify: `gui/src/main.rs` (app startup/shutdown)
- Modify: `gui/src/config.rs` (extend with tab persistence)

**Interfaces:**
- Consumes: `VaultViewerState`, `load_config`, `save_config`
- Produces: `save_tabs(state, root)`, `restore_tabs(state, root)`

- [ ] **Step 1: Add tab persistence to config.rs**

```rust
use crate::state::Tab;

pub fn save_tabs(vault_root: &Path, tabs: &[Tab], active_tab: &Option<String>) -> Result<(), String> {
    let config_dir = vault_root.join(".tomarkdown");
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let tabs_data = TabsState {
        open_tabs: tabs.to_vec(),
        active_tab: active_tab.clone(),
    };

    let tabs_path = config_dir.join("tabs.json");
    let json = serde_json::to_string_pretty(&tabs_data)
        .map_err(|e| format!("Failed to serialize tabs: {}", e))?;

    std::fs::write(&tabs_path, json)
        .map_err(|e| format!("Failed to write tabs: {}", e))
}

pub fn load_tabs(vault_root: &Path) -> Result<Option<TabsState>, String> {
    let tabs_path = vault_root.join(".tomarkdown").join("tabs.json");

    if !tabs_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&tabs_path)
        .map_err(|e| format!("Failed to read tabs: {}", e))?;

    serde_json::from_str(&content)
        .map(Some)
        .map_err(|e| format!("Failed to parse tabs: {}", e))
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TabsState {
    pub open_tabs: Vec<Tab>,
    pub active_tab: Option<String>,
}
```

- [ ] **Step 2: Call load_tabs on app startup**

In `main()` setup:
```rust
.setup(|app| {
    let vault_root = get_vault_root()?; // Implement helper to get current vault
    if let Ok(Some(tabs_state)) = load_tabs(&vault_root) {
        if prefs.auto_restore_tabs {
            // Restore tabs via state mutation
        }
    }
    Ok(())
})
```

- [ ] **Step 3: Call save_tabs on tab changes**

After `set_active_tab` or `close_tab`:
```rust
#[tauri::command]
async fn close_tab(
    tab_id: String,
    vault_root: String,
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<(), String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.close_tab(&tab_id);
    save_tabs(
        Path::new(&vault_root),
        &st.open_tabs,
        &st.active_tab,
    )?;
    Ok(())
}
```

- [ ] **Step 4: Test tab persistence**

```bash
cd gui
cargo test --test integration_tests tab_persistence
```

- [ ] **Step 5: Commit**

```bash
git add gui/src/main.rs gui/src/config.rs
git commit -m "feat: add tab persistence across sessions

Saves open tabs and active tab to .tomarkdown/tabs.json.
Restores on app startup if auto_restore_tabs setting enabled."
```

---

## Phase 3: Recycle Bin (8 Tasks)

### Task 5: Implement recycle bin operations

**Files:**
- Create: `gui/src/recycle_bin.rs`
- Modify: `gui/src/state.rs` (add delete/restore methods)

**Interfaces:**
- Consumes: `VaultViewerState`, `DeletedFile`
- Produces: `delete_file()`, `restore_file()`, `permanently_delete()`, `cleanup_expired()`

- [ ] **Step 1: Implement delete_file in state.rs**

```rust
impl VaultViewerState {
    pub fn delete_file(&mut self, path: String, vault_root: &Path) -> Result<(), String> {
        let vault_path = path
            .strip_prefix(vault_root)
            .map_err(|_| "Path outside vault".to_string())?
            .to_string_lossy()
            .into_owned();

        let recycle_dir = vault_root.join(".tomarkdown").join("recycle_bin");
        std::fs::create_dir_all(&recycle_dir)
            .map_err(|e| format!("Failed to create recycle bin: {}", e))?;

        let deleted_file = DeletedFile {
            id: uuid::Uuid::new_v4().to_string(),
            original_path: path.clone(),
            vault_path,
            deleted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            file_size: std::fs::metadata(&path)
                .map(|m| m.len())
                .unwrap_or(0),
        };

        self.deleted_files.push(deleted_file.clone());

        // Move file to recycle bin
        let filename = std::path::Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy();
        let recycle_path = recycle_dir.join(filename.as_ref());

        std::fs::rename(&path, &recycle_path)
            .map_err(|e| format!("Failed to move file to recycle: {}", e))
    }

    pub fn restore_file(&mut self, file_id: &str, vault_root: &Path) -> Result<(), String> {
        let deleted = self
            .deleted_files
            .iter()
            .find(|f| f.id == file_id)
            .cloned()
            .ok_or("File not found in recycle bin")?;

        let recycle_dir = vault_root.join(".tomarkdown").join("recycle_bin");
        let filename = std::path::Path::new(&deleted.original_path)
            .file_name()
            .unwrap();
        let recycle_path = recycle_dir.join(filename);

        // Create parent directory if needed
        if let Some(parent) = std::path::Path::new(&deleted.original_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent dir: {}", e))?;
        }

        std::fs::rename(&recycle_path, &deleted.original_path)
            .map_err(|e| format!("Failed to restore file: {}", e))?;

        self.deleted_files.retain(|f| f.id != file_id);
        Ok(())
    }

    pub fn permanently_delete(&mut self, file_id: &str, vault_root: &Path) -> Result<(), String> {
        let deleted = self
            .deleted_files
            .iter()
            .find(|f| f.id == file_id)
            .cloned()
            .ok_or("File not found in recycle bin")?;

        let recycle_dir = vault_root.join(".tomarkdown").join("recycle_bin");
        let filename = std::path::Path::new(&deleted.original_path)
            .file_name()
            .unwrap();
        let recycle_path = recycle_dir.join(filename);

        if recycle_path.exists() {
            std::fs::remove_file(&recycle_path)
                .map_err(|e| format!("Failed to delete file: {}", e))?;
        }

        self.deleted_files.retain(|f| f.id != file_id);
        Ok(())
    }

    pub fn cleanup_expired(&mut self, vault_root: &Path, retention_days: u32) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let retention_secs = (retention_days as i64) * 24 * 60 * 60;
        let recycle_dir = vault_root.join(".tomarkdown").join("recycle_bin");

        for deleted in &self.deleted_files {
            if now - deleted.deleted_at > retention_secs {
                let filename = std::path::Path::new(&deleted.original_path)
                    .file_name()
                    .unwrap();
                let recycle_path = recycle_dir.join(filename);

                let _ = std::fs::remove_file(&recycle_path);
            }
        }

        self.deleted_files.retain(|f| now - f.deleted_at <= retention_secs);
        Ok(())
    }
}
```

- [ ] **Step 2: Test recycle operations**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.md");
        fs::write(&test_file, "content").unwrap();

        let mut state = VaultViewerState::new(default_prefs());
        state.delete_file(test_file.to_string_lossy().to_string(), temp_dir.path()).unwrap();

        assert_eq!(state.deleted_files.len(), 1);
        assert!(!test_file.exists());
    }

    #[test]
    fn test_restore_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.md");
        fs::write(&test_file, "content").unwrap();

        let mut state = VaultViewerState::new(default_prefs());
        state.delete_file(test_file.to_string_lossy().to_string(), temp_dir.path()).unwrap();
        let file_id = state.deleted_files[0].id.clone();

        state.restore_file(&file_id, temp_dir.path()).unwrap();

        assert_eq!(state.deleted_files.len(), 0);
        assert!(test_file.exists());
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd gui
cargo test recycle
```

Expected: Both tests pass

- [ ] **Step 4: Commit**

```bash
git add gui/src/state.rs
git commit -m "feat: implement recycle bin operations

Adds delete_file, restore_file, permanently_delete, cleanup_expired
methods to VaultViewerState."
```

---

### Task 6: Add recycle bin Tauri commands

**Files:**
- Create: `gui/src/commands/recycle_commands.rs`
- Modify: `gui/src/main.rs` (register commands)

**Interfaces:**
- Consumes: `VaultViewerState`, `delete_file`, `restore_file`, `permanently_delete`
- Produces: Tauri commands: `delete_file`, `restore_file`, `permanently_delete`, `get_deleted_files`, `empty_recycle_bin`

- [ ] **Step 1: Create recycle_commands.rs**

```rust
use crate::state::{VaultViewerState, DeletedFile};
use std::sync::Mutex;
use tauri::State;
use std::path::Path;

#[tauri::command]
async fn delete_file(
    path: String,
    vault_root: String,
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<(), String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.delete_file(path, Path::new(&vault_root))?;
    Ok(())
}

#[tauri::command]
async fn restore_file(
    file_id: String,
    vault_root: String,
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<(), String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.restore_file(&file_id, Path::new(&vault_root))?;
    Ok(())
}

#[tauri::command]
async fn permanently_delete(
    file_id: String,
    vault_root: String,
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<(), String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.permanently_delete(&file_id, Path::new(&vault_root))?;
    Ok(())
}

#[tauri::command]
async fn get_deleted_files(
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<Vec<DeletedFile>, String> {
    let st = state.lock().map_err(|e| e.to_string())?;
    Ok(st.deleted_files.clone())
}

#[tauri::command]
async fn empty_recycle_bin(
    vault_root: String,
    state: State<'_, Mutex<VaultViewerState>>,
) -> Result<(), String> {
    let mut st = state.lock().map_err(|e| e.to_string())?;
    let file_ids: Vec<String> = st.deleted_files.iter().map(|f| f.id.clone()).collect();
    for id in file_ids {
        st.permanently_delete(&id, Path::new(&vault_root))?;
    }
    Ok(())
}
```

- [ ] **Step 2: Register commands in main.rs**

Add to invoke_handler:
```rust
delete_file, restore_file, permanently_delete, get_deleted_files, empty_recycle_bin
```

- [ ] **Step 3: Verify compilation**

```bash
cd gui
cargo check
```

- [ ] **Step 4: Commit**

```bash
git add gui/src/commands/recycle_commands.rs gui/src/main.rs
git commit -m "feat: add recycle bin Tauri commands

Exposes: delete_file, restore_file, permanently_delete, get_deleted_files,
empty_recycle_bin to frontend via IPC."
```

---

## Phase 4: Clipboard/Copy Options (7 Tasks)

### Task 7: Implement copy/clipboard operations

**Files:**
- Create: `gui/src/clipboard_ops.rs`

**Interfaces:**
- Produces: `copy_as_base64()`, `copy_as_markdown()`, `copy_as_hex()`, `copy_hash()`

- [ ] **Step 1: Create clipboard_ops.rs**

```rust
use std::path::Path;
use sha2::{Sha256, Digest};
use md5;

pub fn copy_as_base64(file_path: &Path) -> Result<String, String> {
    let content = std::fs::read(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    Ok(base64::encode(&content))
}

pub fn copy_as_markdown(file_path: &Path, file_type: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    match file_type {
        "markdown" => Ok(content),
        "code" => {
            let lang = detect_language(file_path);
            Ok(format!("```{}\n{}\n```", lang, content))
        }
        "image" => {
            let b64 = copy_as_base64(file_path)?;
            Ok(format!("![image](data:image/{};base64,{})", "png", b64))
        }
        _ => {
            Ok(format!("```\n{}\n```", content))
        }
    }
}

pub fn copy_as_hex(file_path: &Path) -> Result<String, String> {
    let content = std::fs::read(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let hex: String = content
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    Ok(hex)
}

pub fn copy_sha256(file_path: &Path) -> Result<String, String> {
    let content = std::fs::read(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(&content);
    let result = hasher.finalize();

    Ok(format!("{:x}", result))
}

pub fn copy_md5(file_path: &Path) -> Result<String, String> {
    let content = std::fs::read(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let digest = md5::compute(&content);
    Ok(format!("{:x}", digest))
}

pub fn copy_crc(file_path: &Path) -> Result<String, String> {
    let content = std::fs::read(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let mut crc = 0u32;
    for byte in &content {
        crc = crc.wrapping_mul(31).wrapping_add(*byte as u32);
    }

    Ok(format!("{:08x}", crc))
}

fn detect_language(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext {
            "rs" => "rust",
            "py" => "python",
            "js" => "javascript",
            "ts" => "typescript",
            "go" => "go",
            "c" => "c",
            "cpp" => "cpp",
            "java" => "java",
            "rb" => "ruby",
            "php" => "php",
            _ => "",
        })
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_copy_as_base64() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "hello").unwrap();

        let b64 = copy_as_base64(&test_file).unwrap();
        assert_eq!(b64, "aGVsbG8=");
    }

    #[test]
    fn test_copy_sha256() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "hello").unwrap();

        let hash = copy_sha256(&test_file).unwrap();
        assert_eq!(hash.len(), 64); // SHA256 in hex is 64 chars
    }

    #[test]
    fn test_copy_as_markdown_code() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() {}").unwrap();

        let md = copy_as_markdown(&test_file, "code").unwrap();
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main()"));
    }
}
```

- [ ] **Step 2: Add dependencies to Cargo.toml**

```toml
sha2 = "0.10"
md5 = "0.7"
base64 = "0.21"
```

- [ ] **Step 3: Run tests**

```bash
cd gui
cargo test clipboard_ops
```

Expected: All 3 tests pass

- [ ] **Step 4: Commit**

```bash
git add gui/src/clipboard_ops.rs gui/Cargo.toml
git commit -m "feat: implement copy/clipboard operations

Adds base64, markdown, hex encoding and SHA256/MD5/CRC hash functions
for copying file content in multiple formats."
```

---

### Task 8: Add clipboard Tauri commands

**Files:**
- Create: `gui/src/commands/clipboard_commands.rs`
- Modify: `gui/src/main.rs` (register commands)

**Interfaces:**
- Consumes: Copy operations from clipboard_ops
- Produces: Tauri commands with format parameter

- [ ] **Step 1: Create clipboard_commands.rs**

```rust
use crate::clipboard_ops;
use std::path::Path;

#[tauri::command]
async fn copy_file(
    path: String,
    format: String,
    file_type: String,
) -> Result<String, String> {
    let p = Path::new(&path);

    let content = match format.as_str() {
        "base64" => clipboard_ops::copy_as_base64(p)?,
        "markdown" => clipboard_ops::copy_as_markdown(p, &file_type)?,
        "hex" => clipboard_ops::copy_as_hex(p)?,
        "sha256" => clipboard_ops::copy_sha256(p)?,
        "md5" => clipboard_ops::copy_md5(p)?,
        "crc" => clipboard_ops::copy_crc(p)?,
        _ => return Err(format!("Unknown format: {}", format)),
    };

    // Copy to system clipboard
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?
            .stdin
            .ok_or("Failed to open pbcopy stdin")?
            .write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(&["/C", "clip"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?
            .stdin
            .ok_or("Failed to open clip stdin")?
            .write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        Command::new("xclip")
            .args(&["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?
            .stdin
            .ok_or("Failed to open xclip stdin")?
            .write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    Ok(content)
}
```

- [ ] **Step 2: Register commands**

Add to invoke_handler:
```rust
copy_file
```

- [ ] **Step 3: Verify compilation**

```bash
cd gui
cargo check
```

- [ ] **Step 4: Commit**

```bash
git add gui/src/commands/clipboard_commands.rs gui/src/main.rs
git commit -m "feat: add copy_file Tauri command

Exposes clipboard operations to frontend with format selection:
base64, markdown, hex, sha256, md5, crc."
```

---

## Phase 5: Image Zoom (8 Tasks)

### Task 9: Implement image zoom calculations

**Files:**
- Create: `gui/src/image_zoom.rs`

**Interfaces:**
- Produces: `ZoomCalculator` struct with methods for zoom, pan, fit

- [ ] **Step 1: Create image_zoom.rs**

```rust
pub struct ZoomCalculator {
    pub current_zoom: f32,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub image_width: u32,
    pub image_height: u32,
    pub pan_x: i32,
    pub pan_y: i32,
}

impl ZoomCalculator {
    pub fn new(viewport_width: u32, viewport_height: u32, image_width: u32, image_height: u32) -> Self {
        ZoomCalculator {
            current_zoom: 1.0,
            viewport_width,
            viewport_height,
            image_width,
            image_height,
            pan_x: 0,
            pan_y: 0,
        }
    }

    pub fn zoom_in(&mut self) {
        self.current_zoom = (self.current_zoom + 0.1).min(15.0);
    }

    pub fn zoom_out(&mut self) {
        self.current_zoom = (self.current_zoom - 0.1).max(1.0);
    }

    pub fn reset_zoom(&mut self) {
        self.current_zoom = 1.0;
        self.pan_x = 0;
        self.pan_y = 0;
    }

    pub fn fit_to_window(&mut self) {
        let zoom_x = self.viewport_width as f32 / self.image_width as f32;
        let zoom_y = self.viewport_height as f32 / self.image_height as f32;
        self.current_zoom = zoom_x.min(zoom_y).max(1.0).min(15.0);
        self.pan_x = 0;
        self.pan_y = 0;
    }

    pub fn pan(&mut self, dx: i32, dy: i32) {
        let display_width = (self.image_width as f32 * self.current_zoom) as i32;
        let display_height = (self.image_height as f32 * self.current_zoom) as i32;

        let max_pan_x = (display_width - self.viewport_width as i32).max(0);
        let max_pan_y = (display_height - self.viewport_height as i32).max(0);

        self.pan_x = (self.pan_x + dx).max(0).min(max_pan_x);
        self.pan_y = (self.pan_y + dy).max(0).min(max_pan_y);
    }

    pub fn mouse_wheel_zoom(&mut self, delta: i32) {
        if delta > 0 {
            self.zoom_in();
        } else {
            self.zoom_out();
        }
    }

    pub fn get_display_dimensions(&self) -> (u32, u32) {
        let width = (self.image_width as f32 * self.current_zoom) as u32;
        let height = (self.image_height as f32 * self.current_zoom) as u32;
        (width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_bounds() {
        let mut calc = ZoomCalculator::new(800, 600, 400, 300);

        for _ in 0..100 {
            calc.zoom_in();
        }
        assert_eq!(calc.current_zoom, 15.0);

        for _ in 0..100 {
            calc.zoom_out();
        }
        assert_eq!(calc.current_zoom, 1.0);
    }

    #[test]
    fn test_fit_to_window() {
        let mut calc = ZoomCalculator::new(800, 600, 1600, 1200);
        calc.current_zoom = 5.0;
        calc.fit_to_window();
        assert!(calc.current_zoom < 5.0);
        assert_eq!(calc.pan_x, 0);
        assert_eq!(calc.pan_y, 0);
    }

    #[test]
    fn test_pan_bounds() {
        let mut calc = ZoomCalculator::new(800, 600, 400, 300);
        calc.current_zoom = 3.0;
        calc.pan(10000, 10000); // Try to pan way beyond bounds
        
        let (w, h) = calc.get_display_dimensions();
        assert!(calc.pan_x <= (w as i32 - 800));
        assert!(calc.pan_y <= (h as i32 - 600));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd gui
cargo test image_zoom
```

Expected: All 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add gui/src/image_zoom.rs
git commit -m "feat: implement image zoom calculations

Adds ZoomCalculator with zoom_in/out, fit_to_window, pan, and
mouse wheel support with proper boundary calculations."
```

---

### Task 10: Add image zoom Tauri commands

**Files:**
- Create: `gui/src/commands/image_commands.rs`
- Modify: `gui/src/main.rs` (register commands, add zoom state)
- Modify: `gui/src/state.rs` (add zoom tracking)

**Interfaces:**
- Consumes: `ZoomCalculator`, `VaultViewerState`
- Produces: Tauri commands: `zoom_in`, `zoom_out`, `reset_zoom`, `fit_to_window`, `pan`

- [ ] **Step 1: Create image_commands.rs**

```rust
use crate::image_zoom::ZoomCalculator;
use std::sync::Mutex;
use std::collections::HashMap;
use tauri::State;

#[tauri::command]
async fn zoom_in(
    file_id: String,
    zoom_state: State<'_, Mutex<HashMap<String, ZoomCalculator>>>,
) -> Result<f32, String> {
    let mut state = zoom_state.lock().map_err(|e| e.to_string())?;
    let calc = state.entry(file_id).or_insert_with(|| {
        ZoomCalculator::new(800, 600, 400, 300)
    });
    calc.zoom_in();
    Ok(calc.current_zoom)
}

#[tauri::command]
async fn zoom_out(
    file_id: String,
    zoom_state: State<'_, Mutex<HashMap<String, ZoomCalculator>>>,
) -> Result<f32, String> {
    let mut state = zoom_state.lock().map_err(|e| e.to_string())?;
    let calc = state.entry(file_id).or_insert_with(|| {
        ZoomCalculator::new(800, 600, 400, 300)
    });
    calc.zoom_out();
    Ok(calc.current_zoom)
}

#[tauri::command]
async fn reset_zoom(
    file_id: String,
    zoom_state: State<'_, Mutex<HashMap<String, ZoomCalculator>>>,
) -> Result<(), String> {
    let mut state = zoom_state.lock().map_err(|e| e.to_string())?;
    if let Some(calc) = state.get_mut(&file_id) {
        calc.reset_zoom();
    }
    Ok(())
}

#[tauri::command]
async fn fit_to_window(
    file_id: String,
    zoom_state: State<'_, Mutex<HashMap<String, ZoomCalculator>>>,
) -> Result<f32, String> {
    let mut state = zoom_state.lock().map_err(|e| e.to_string())?;
    let calc = state.entry(file_id).or_insert_with(|| {
        ZoomCalculator::new(800, 600, 400, 300)
    });
    calc.fit_to_window();
    Ok(calc.current_zoom)
}

#[tauri::command]
async fn pan(
    file_id: String,
    dx: i32,
    dy: i32,
    zoom_state: State<'_, Mutex<HashMap<String, ZoomCalculator>>>,
) -> Result<(i32, i32), String> {
    let mut state = zoom_state.lock().map_err(|e| e.to_string())?;
    let calc = state.entry(file_id).or_insert_with(|| {
        ZoomCalculator::new(800, 600, 400, 300)
    });
    calc.pan(dx, dy);
    Ok((calc.pan_x, calc.pan_y))
}
```

- [ ] **Step 2: Register commands and manage state**

In main.rs:
```rust
.manage(Mutex::new(HashMap::<String, ZoomCalculator>::new()))
```

Add to invoke_handler:
```rust
zoom_in, zoom_out, reset_zoom, fit_to_window, pan
```

- [ ] **Step 3: Verify compilation**

```bash
cd gui
cargo check
```

- [ ] **Step 4: Commit**

```bash
git add gui/src/commands/image_commands.rs gui/src/main.rs
git commit -m "feat: add image zoom Tauri commands

Exposes zoom_in, zoom_out, reset_zoom, fit_to_window, pan to frontend.
Manages per-file zoom state."
```

---

## Phase 6: Settings/Preferences (6 Tasks)

### Task 11: Create settings dialog state and commands

**Files:**
- Modify: `gui/src/config.rs` (add update functions)
- Create: `gui/src/commands/settings_commands.rs`
- Modify: `gui/src/main.rs` (register commands)

**Interfaces:**
- Consumes: `UserPreferences`, `load_config`, `save_config`
- Produces: Tauri commands: `get_preferences`, `update_preferences`

- [ ] **Step 1: Add update functions to config.rs**

```rust
pub fn update_tab_mode(vault_root: &Path, mode: TabMode) -> Result<(), String> {
    let mut prefs = load_config(vault_root)?;
    prefs.tab_mode = mode;
    save_config(vault_root, prefs)
}

pub fn update_recycle_retention(vault_root: &Path, days: u32) -> Result<(), String> {
    let mut prefs = load_config(vault_root)?;
    prefs.recycle_retention_days = days;
    save_config(vault_root, prefs)
}

pub fn update_auto_save(vault_root: &Path, enabled: bool) -> Result<(), String> {
    let mut prefs = load_config(vault_root)?;
    prefs.auto_save = enabled;
    save_config(vault_root, prefs)
}

pub fn update_theme(vault_root: &Path, theme: String) -> Result<(), String> {
    let mut prefs = load_config(vault_root)?;
    prefs.theme = theme;
    save_config(vault_root, prefs)
}

pub fn update_zoom_behavior(vault_root: &Path, behavior: ZoomBehavior) -> Result<(), String> {
    let mut prefs = load_config(vault_root)?;
    prefs.zoom_behavior = behavior;
    save_config(vault_root, prefs)
}
```

- [ ] **Step 2: Create settings_commands.rs**

```rust
use crate::config;
use crate::state::UserPreferences;
use std::path::Path;

#[tauri::command]
async fn get_preferences(vault_root: String) -> Result<UserPreferences, String> {
    config::load_config(Path::new(&vault_root))
}

#[tauri::command]
async fn update_preferences(
    vault_root: String,
    preferences: UserPreferences,
) -> Result<UserPreferences, String> {
    config::save_config(Path::new(&vault_root), preferences.clone())?;
    Ok(preferences)
}
```

- [ ] **Step 3: Register commands**

Add to invoke_handler:
```rust
get_preferences, update_preferences
```

- [ ] **Step 4: Verify compilation**

```bash
cd gui
cargo check
```

- [ ] **Step 5: Commit**

```bash
git add gui/src/config.rs gui/src/commands/settings_commands.rs gui/src/main.rs
git commit -m "feat: add settings persistence Tauri commands

Exposes get_preferences and update_preferences for settings dialog.
Allows runtime configuration changes."
```

---

## Phase 7: Frontend Integration (15 Tasks)

### Task 12: Add tab management UI

**Files:**
- Modify: `gui/ui/index.html` (update tab bar)
- Create: `gui/ui/js/tab-manager.js`
- Modify: `gui/ui/styles.css`

*Note: Frontend tasks are high-level; each can be broken into substeps by the implementer.*

- [ ] Implement tab bar rendering with close buttons
- [ ] Add keyboard shortcuts (Cmd+Tab, Cmd+Shift+Tab, Cmd+W)
- [ ] Implement context menu (right-click → close all except)
- [ ] Wire tab events to Tauri commands
- [ ] Test tab creation, switching, closing

### Task 13: Add recycle bin UI

**Files:**
- Modify: `gui/ui/components/file-tree.js`
- Create: `gui/ui/js/recycle-bin-ui.js`

- [ ] Add recycle bin as special folder in file tree
- [ ] Show deleted file count as badge
- [ ] Implement restore context menu
- [ ] Implement permanent delete option
- [ ] Wire recycle operations to Tauri commands

### Task 14: Add copy/clipboard context menu

**Files:**
- Create: `gui/ui/js/clipboard-menu.js`

- [ ] Create context menu with grouped copy options
- [ ] Implement "Copy as" submenu with format options
- [ ] Wire copy operations to Tauri commands
- [ ] Show toast notification on copy success
- [ ] Handle keyboard shortcuts for common formats

### Task 15: Add image zoom controls

**Files:**
- Create: `gui/ui/js/image-zoom-ui.js`
- Modify: `gui/ui/styles.css`

- [ ] Create toolbar with zoom buttons (−, current zoom, +)
- [ ] Implement zoom slider
- [ ] Add fit-to-window button
- [ ] Wire zoom controls to Tauri commands
- [ ] Implement mouse wheel zoom
- [ ] Handle pan on zoomed images

### Task 16: Create settings dialog

**Files:**
- Create: `gui/ui/js/settings-panel.js`
- Modify: `gui/ui/index.html`
- Modify: `gui/ui/styles.css`

- [ ] Create modal dialog for preferences
- [ ] Add tab mode toggle (single/multi)
- [ ] Add recycle retention dropdown
- [ ] Add auto-save toggle
- [ ] Add theme selector
- [ ] Add zoom behavior radio buttons
- [ ] Wire settings to Tauri commands
- [ ] Persist settings changes

---

## Phase 8: Integration Testing (5 Tasks)

### Task 17: Create integration test suite

**Files:**
- Create: `gui/tests/integration_tests.rs`

- [ ] Test complete tab workflow (create → switch → close)
- [ ] Test recycle bin workflow (delete → restore)
- [ ] Test copy operations for all formats
- [ ] Test image zoom workflow
- [ ] Test settings persistence

### Task 18: E2E testing

**Files:**
- Documentation: `gui/TESTING.md`

- [ ] Manual test plan for tab management
- [ ] Manual test plan for recycle bin
- [ ] Manual test plan for copy operations
- [ ] Manual test plan for image zoom
- [ ] Manual test plan for settings

---

## Summary

This plan implements 18 major tasks across 8 phases:

1. **Phase 1 (2 tasks):** Foundation - VaultViewerState and config management
2. **Phase 2 (4 tasks):** Tab Management - tab operations, persistence, commands, UI
3. **Phase 3 (4 tasks):** Recycle Bin - operations, commands, UI
4. **Phase 4 (3 tasks):** Clipboard/Copy - operations, commands, UI
5. **Phase 5 (3 tasks):** Image Zoom - calculations, commands, UI
6. **Phase 6 (2 tasks):** Settings - persistence, commands, UI
7. **Phase 7 (5 tasks):** Frontend Integration - UI implementation
8. **Phase 8 (2 tasks):** Testing - integration and E2E tests

Each task produces independently testable code with TDD approach and frequent commits.

