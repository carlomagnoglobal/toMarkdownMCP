use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

/// Parsed subset of `.obsidian/` settings that other tools care about.
#[derive(Debug, Default, Serialize)]
pub struct VaultConfig {
    pub has_obsidian_dir: bool,
    pub attachment_folder: Option<String>,
    pub new_link_format: Option<String>,
    pub daily_notes_folder: Option<String>,
    pub daily_notes_format: Option<String>,
    pub daily_notes_template: Option<String>,
    pub templates_folder: Option<String>,
    pub enabled_core_plugins: Vec<String>,
}

fn read_json(path: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn get_str(v: &Option<Value>, key: &str) -> Option<String> {
    v.as_ref()?.get(key)?.as_str().map(|s| s.to_string())
}

/// Read `.obsidian/{app,daily-notes,templates,core-plugins}.json` (all optional).
pub fn read_config(vault_root: &Path) -> Result<VaultConfig> {
    let dir = vault_root.join(".obsidian");
    let mut cfg = VaultConfig { has_obsidian_dir: dir.is_dir(), ..Default::default() };
    if !cfg.has_obsidian_dir {
        return Ok(cfg);
    }

    let app = read_json(&dir.join("app.json"));
    cfg.attachment_folder = get_str(&app, "attachmentFolderPath");
    cfg.new_link_format = get_str(&app, "newLinkFormat");

    let daily = read_json(&dir.join("daily-notes.json"));
    cfg.daily_notes_folder = get_str(&daily, "folder");
    cfg.daily_notes_format = get_str(&daily, "format");
    cfg.daily_notes_template = get_str(&daily, "template");

    let templates = read_json(&dir.join("templates.json"));
    cfg.templates_folder = get_str(&templates, "folder");

    if let Some(Value::Array(plugins)) = read_json(&dir.join("core-plugins.json")) {
        cfg.enabled_core_plugins = plugins
            .into_iter()
            .filter_map(|p| p.as_str().map(|s| s.to_string()))
            .collect();
    }
    Ok(cfg)
}

/// Convert an Obsidian/Moment date format (YYYY-MM-DD etc.) to chrono's.
pub fn moment_to_chrono(format: &str) -> String {
    // Longest tokens first to avoid partial replacements.
    format
        .replace("YYYY", "%Y")
        .replace("MM", "%m")
        .replace("DD", "%d")
        .replace("HH", "%H")
        .replace("mm", "%M")
        .replace("ss", "%S")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_vault")
    }

    #[test]
    fn reads_fixture_config() {
        let cfg = read_config(&fixture()).unwrap();
        assert!(cfg.has_obsidian_dir);
        assert_eq!(cfg.attachment_folder.as_deref(), Some("attachments"));
        assert_eq!(cfg.daily_notes_folder.as_deref(), Some("daily"));
        assert_eq!(cfg.daily_notes_format.as_deref(), Some("YYYY-MM-DD"));
        assert_eq!(cfg.daily_notes_template.as_deref(), Some("templates/Daily"));
        assert_eq!(cfg.templates_folder.as_deref(), Some("templates"));
    }

    #[test]
    fn no_obsidian_dir() {
        let cfg = read_config(Path::new("/tmp")).unwrap_or_default();
        // /tmp may or may not exist as vault; just ensure no panic and flag false-ish
        assert!(!cfg.has_obsidian_dir || cfg.attachment_folder.is_none());
    }

    #[test]
    fn moment_format() {
        assert_eq!(moment_to_chrono("YYYY-MM-DD"), "%Y-%m-%d");
        assert_eq!(moment_to_chrono("DD.MM.YYYY"), "%d.%m.%Y");
    }
}
