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
