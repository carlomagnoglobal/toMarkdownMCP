use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;

use super::vault::{self, Resolution, VaultIndex};
use super::wikilink::{parse_wikilinks, WikiLink};

/// Normalize a user-supplied link string: strips `![[`, `[[`, `]]`.
fn parse_link_arg(link: &str) -> Result<WikiLink> {
    let trimmed = link.trim();
    let wrapped = if trimmed.starts_with("[[") || trimmed.starts_with("![[") {
        trimmed.to_string()
    } else {
        format!("[[{}]]", trimmed)
    };
    parse_wikilinks(&wrapped)
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Could not parse link: {}", link))
}

/// Resolve `note` (path, stem, or alias) to an existing note path in the index.
pub fn resolve_note_arg(idx: &VaultIndex, note: &str) -> Result<String> {
    if idx.notes.contains_key(note) {
        return Ok(note.to_string());
    }
    match vault::resolve_target(idx, note.trim_end_matches(".md"), None) {
        Resolution::Resolved(p) if idx.notes.contains_key(&p) => Ok(p),
        Resolution::Resolved(p) => Err(anyhow!("'{}' resolves to attachment {}, not a note", note, p)),
        Resolution::Ambiguous(c) => Err(anyhow!(
            "'{}' is ambiguous; candidates: {}. Use a vault-relative path.",
            note,
            c.join(", ")
        )),
        Resolution::Broken => Err(anyhow!("Note not found: {}", note)),
    }
}

fn fm_to_json(fm: &serde_yaml::Value) -> Value {
    serde_json::to_value(fm).unwrap_or(Value::Null)
}

// ---- vault_index ----

pub fn vault_index(root: &Path, include_orphans: bool) -> Result<Value> {
    let idx = vault::get_index(root)?;

    let mut broken: Vec<Value> = Vec::new();
    let mut ambiguous: Vec<Value> = Vec::new();
    for (from, links) in &idx.links {
        for l in links {
            match vault::resolve_target(&idx, &l.target, Some(from)) {
                Resolution::Broken => broken.push(json!({"from": from, "link": l.raw, "line": l.line})),
                Resolution::Ambiguous(c) => {
                    ambiguous.push(json!({"from": from, "link": l.raw, "candidates": c}))
                }
                Resolution::Resolved(_) => {}
            }
        }
    }

    let mut tag_counts: Vec<(String, usize)> =
        idx.tags.iter().map(|(t, v)| (t.clone(), v.len())).collect();
    tag_counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let mut result = json!({
        "vault": root.display().to_string(),
        "note_count": idx.notes.len(),
        "attachment_count": idx.attachments.len(),
        "tags": tag_counts.iter().map(|(t, c)| json!({"tag": t, "count": c})).collect::<Vec<_>>(),
        "aliases": idx.aliases.iter().map(|(a, paths)| json!({"alias": a, "notes": paths})).collect::<Vec<_>>(),
        "broken_links": broken,
        "ambiguous_links": ambiguous,
    });

    if include_orphans {
        let mut orphans: Vec<&String> = idx
            .notes
            .keys()
            .filter(|p| !idx.backlinks.contains_key(*p))
            .collect();
        orphans.sort();
        result["orphans"] = json!(orphans);
    }
    Ok(result)
}

// ---- get_note ----

pub fn get_note(
    root: &Path,
    note: &str,
    inline_embeds: bool,
    max_embed_depth: usize,
) -> Result<Value> {
    let idx = vault::get_index(root)?;
    let path = resolve_note_arg(&idx, note)?;
    let meta = &idx.notes[&path];
    let content = std::fs::read_to_string(root.join(&path))?;
    let (_, body) = super::frontmatter::split(&content);

    let body = if inline_embeds {
        transclude(&idx, root, body, &path, max_embed_depth)?
    } else {
        body.to_string()
    };

    Ok(json!({
        "path": path,
        "title": meta.title,
        "frontmatter": fm_to_json(&meta.frontmatter),
        "aliases": meta.aliases,
        "tags": meta.tags,
        "headings": meta.headings.iter().map(|(l, t)| json!({"level": l, "text": t})).collect::<Vec<_>>(),
        "links": idx.links.get(&path),
        "backlink_count": idx.backlinks.get(&path).map(|b| b.len()).unwrap_or(0),
        "content": body,
    }))
}

/// Replace `![[note]]` / `![[note#heading]]` embeds with the target content.
fn transclude(
    idx: &VaultIndex,
    root: &Path,
    body: &str,
    from: &str,
    depth: usize,
) -> Result<String> {
    if depth == 0 {
        return Ok(body.to_string());
    }
    let links: Vec<WikiLink> = parse_wikilinks(body).into_iter().filter(|l| l.embed).collect();
    if links.is_empty() {
        return Ok(body.to_string());
    }
    let mut out = body.to_string();
    for link in links {
        let Resolution::Resolved(target) = vault::resolve_target(idx, &link.target, Some(from))
        else {
            continue;
        };
        if !idx.notes.contains_key(&target) {
            continue; // attachment embed (image etc.) — leave as-is
        }
        let content = std::fs::read_to_string(root.join(&target))?;
        let (_, target_body) = super::frontmatter::split(&content);
        let section = match &link.heading {
            Some(h) => heading_section(target_body, h),
            None => target_body.to_string(),
        };
        let inlined = transclude(idx, root, &section, &target, depth - 1)?;
        let replacement = format!(
            "> [!quote] {}\n{}",
            link.raw.trim_start_matches('!'),
            inlined.trim_end().lines().map(|l| format!("> {}", l)).collect::<Vec<_>>().join("\n")
        );
        out = out.replace(&link.raw, &replacement);
    }
    Ok(out)
}

/// Extract the section of `body` under heading `h` (up to the next heading of
/// the same or higher level).
fn heading_section(body: &str, h: &str) -> String {
    let mut out = Vec::new();
    let mut level = 0usize;
    let mut capturing = false;
    for line in body.lines() {
        let t = line.trim_start();
        let hashes = t.chars().take_while(|&c| c == '#').count();
        let is_heading = (1..=6).contains(&hashes) && t.chars().nth(hashes) == Some(' ');
        if is_heading {
            let text = t[hashes + 1..].trim();
            if capturing && hashes <= level {
                break;
            }
            if !capturing && text.eq_ignore_ascii_case(h) {
                capturing = true;
                level = hashes;
            }
        }
        if capturing {
            out.push(line);
        }
    }
    if out.is_empty() {
        body.to_string()
    } else {
        out.join("\n")
    }
}

// ---- resolve_link ----

pub fn resolve_link(root: &Path, link: &str, from_note: Option<&str>) -> Result<Value> {
    let idx = vault::get_index(root)?;
    let parsed = parse_link_arg(link)?;
    let from = match from_note {
        Some(n) => Some(resolve_note_arg(&idx, n)?),
        None => None,
    };
    let resolution = vault::resolve_target(&idx, &parsed.target, from.as_deref());

    let mut result = json!({
        "link": parsed,
        "resolution": resolution,
    });

    if let Resolution::Resolved(path) = &resolution {
        // Locate heading / block line if requested.
        if let Some(h) = &parsed.heading {
            if let Some(meta) = idx.notes.get(path) {
                let found = meta.headings.iter().any(|(_, text)| text.eq_ignore_ascii_case(h));
                result["heading_found"] = json!(found);
            }
        }
        if let Some(b) = &parsed.block {
            let line = idx.blocks.get(path).and_then(|m| m.get(b));
            result["block_line"] = json!(line);
        }
    }
    Ok(result)
}

// ---- get_backlinks ----

pub fn get_backlinks(root: &Path, note: &str) -> Result<Value> {
    let idx = vault::get_index(root)?;
    let path = resolve_note_arg(&idx, note)?;
    let backlinks = idx.backlinks.get(&path).map(|v| v.as_slice()).unwrap_or(&[]);
    Ok(json!({
        "note": path,
        "backlink_count": backlinks.len(),
        "backlinks": backlinks,
    }))
}

// ---- search ----

pub fn search(root: &Path, query: &str, mode: &str, limit: usize) -> Result<Value> {
    let idx = vault::get_index(root)?;
    let q = query.trim();
    let ql = q.to_lowercase();
    let mut hits: Vec<Value> = Vec::new();

    match mode {
        "tag" => {
            let want = ql.trim_start_matches('#');
            for (tag, paths) in &idx.tags {
                let tl = tag.to_lowercase();
                // exact or nested-prefix match (query "area" matches "area/work")
                if tl == want || tl.starts_with(&format!("{}/", want)) {
                    for p in paths {
                        hits.push(json!({"note": p, "tag": tag}));
                    }
                }
            }
        }
        "alias" => {
            for (alias, paths) in &idx.aliases {
                if alias.contains(&ql) {
                    for p in paths {
                        hits.push(json!({"note": p, "alias": alias}));
                    }
                }
            }
        }
        "field" => {
            // query "key" or "key=value"
            let (key, want_value) = match q.split_once('=') {
                Some((k, v)) => (k.trim(), Some(v.trim().to_lowercase())),
                None => (q, None),
            };
            for (path, meta) in &idx.notes {
                if let Some(v) = meta.frontmatter.get(key) {
                    let v_str = serde_yaml::to_string(v).unwrap_or_default().trim().to_string();
                    let matches = match &want_value {
                        Some(w) => v_str.to_lowercase().contains(w),
                        None => true,
                    };
                    if matches {
                        hits.push(json!({"note": path, "field": key, "value": v_str}));
                    }
                }
            }
        }
        "text" => {
            for path in idx.notes.keys() {
                let Ok(content) = std::fs::read_to_string(root.join(path)) else { continue };
                for (i, line) in content.lines().enumerate() {
                    if line.to_lowercase().contains(&ql) {
                        hits.push(json!({"note": path, "line": i + 1, "context": line.trim()}));
                        if hits.len() >= limit {
                            break;
                        }
                    }
                }
                if hits.len() >= limit {
                    break;
                }
            }
        }
        other => return Err(anyhow!("Unknown search mode '{}' (use tag|alias|field|text)", other)),
    }

    hits.sort_by_key(|h| h["note"].as_str().unwrap_or("").to_string());
    hits.truncate(limit);
    Ok(json!({"query": q, "mode": mode, "hit_count": hits.len(), "hits": hits}))
}

// ---- list_tasks ----

pub fn list_tasks(root: &Path, status: Option<&str>, note: Option<&str>) -> Result<Value> {
    let idx = vault::get_index(root)?;
    let paths: Vec<String> = match note {
        Some(n) => vec![resolve_note_arg(&idx, n)?],
        None => {
            let mut p: Vec<String> = idx.notes.keys().cloned().collect();
            p.sort();
            p
        }
    };

    let mut all = Vec::new();
    let mut counts: std::collections::HashMap<String, usize> = Default::default();
    for path in paths {
        let Ok(content) = std::fs::read_to_string(root.join(&path)) else { continue };
        for task in super::tasks::parse_tasks(&content) {
            *counts.entry(task.status.clone()).or_default() += 1;
            let matches = match status {
                Some(want) => {
                    task.status == want || task.state.to_string() == want
                }
                None => true,
            };
            if matches {
                all.push(json!({"note": path, "task": task}));
            }
        }
    }
    Ok(json!({"task_count": all.len(), "status_counts": counts, "tasks": all}))
}

// ---- get_vault_config ----

pub fn get_vault_config(root: &Path) -> Result<Value> {
    let cfg = super::config::read_config(root)?;
    Ok(serde_json::to_value(&cfg)?)
}

// ---- create_note_from_template ----

pub fn create_note_from_template(
    root: &Path,
    title: Option<&str>,
    template: Option<&str>,
    daily: bool,
) -> Result<Value> {
    let cfg = super::config::read_config(root)?;

    let (title, folder, template_path) = if daily {
        let fmt = super::config::moment_to_chrono(
            cfg.daily_notes_format.as_deref().unwrap_or("YYYY-MM-DD"),
        );
        let name = chrono::Local::now().format(&fmt).to_string();
        (
            name,
            cfg.daily_notes_folder.clone().unwrap_or_default(),
            template.map(|t| t.to_string()).or_else(|| cfg.daily_notes_template.clone()),
        )
    } else {
        let t = title.ok_or_else(|| anyhow!("'title' is required unless daily=true"))?;
        (t.to_string(), String::new(), template.map(|t| t.to_string()))
    };

    // Title may itself carry a folder ("projects/New Note").
    let rel = if folder.is_empty() {
        format!("{}.md", title)
    } else {
        format!("{}/{}.md", folder.trim_end_matches('/'), title)
    };
    let abs = root.join(&rel);
    if abs.exists() {
        return Err(anyhow!("Note already exists: {}", rel));
    }

    let content = match &template_path {
        Some(t) => {
            let mut tp = root.join(t);
            if tp.extension().is_none() {
                tp.set_extension("md");
            }
            let raw = std::fs::read_to_string(&tp)
                .map_err(|e| anyhow!("Cannot read template {}: {}", tp.display(), e))?;
            let stem = title.rsplit('/').next().unwrap_or(&title);
            super::template::render_template(&raw, stem)
        }
        None => format!("# {}\n\n", title.rsplit('/').next().unwrap_or(&title)),
    };

    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&abs, &content)?;
    vault::invalidate(root);

    Ok(json!({"created": rel, "template": template_path, "content": content}))
}

// ---- rename_note ----

pub fn rename_note(root: &Path, note: &str, new_name: &str, dry_run: bool) -> Result<Value> {
    let idx = vault::get_index(root)?;
    let old_path = resolve_note_arg(&idx, note)?;

    // New vault-relative path: explicit path if it contains '/', else same folder.
    let new_rel = {
        let base = new_name.trim_end_matches(".md");
        if base.contains('/') {
            format!("{}.md", base)
        } else {
            match old_path.rsplit_once('/') {
                Some((dir, _)) => format!("{}/{}.md", dir, base),
                None => format!("{}.md", base),
            }
        }
    };
    if root.join(&new_rel).exists() {
        return Err(anyhow!("Target already exists: {}", new_rel));
    }
    let new_sans_ext = new_rel.trim_end_matches(".md").to_string();
    let new_stem = new_sans_ext.rsplit('/').next().unwrap_or(&new_sans_ext).to_string();

    // If the new stem would be ambiguous with another note, use the full path
    // in rewritten links (Obsidian's behavior with "shortest" link format).
    let stem_taken = idx
        .by_stem
        .get(&new_stem.to_lowercase())
        .map(|c| c.iter().any(|p| p != &old_path))
        .unwrap_or(false);
    let new_link_target = if stem_taken { new_sans_ext.clone() } else { new_stem.clone() };

    // Rewrite every inbound link.
    let mut changes: Vec<Value> = Vec::new();
    if let Some(backlinks) = idx.backlinks.get(&old_path) {
        // Group by source file to rewrite each file once.
        let mut by_file: std::collections::HashMap<&str, Vec<&super::vault::Backlink>> =
            Default::default();
        for bl in backlinks {
            by_file.entry(bl.from.as_str()).or_default().push(bl);
        }
        for (file, links) in by_file {
            let abs = root.join(file);
            let content = std::fs::read_to_string(&abs)?;
            let mut updated = content.clone();
            for bl in &links {
                let l = &bl.link;
                // Rebuild the link with the new target, preserving fragments/alias.
                // (Escaped `\|` aliases are rewritten with plain `|`.)
                let mut inner = new_link_target.clone();
                if let Some(h) = &l.heading {
                    inner.push('#');
                    inner.push_str(h);
                }
                if let Some(b) = &l.block {
                    inner.push_str("#^");
                    inner.push_str(b);
                }
                if let Some(a) = &l.alias {
                    inner.push('|');
                    inner.push_str(a);
                }
                let new_raw = format!("{}[[{}]]", if l.embed { "!" } else { "" }, inner);
                if l.raw != new_raw {
                    updated = updated.replace(&l.raw, &new_raw);
                    changes.push(json!({"file": file, "old": l.raw, "new": new_raw, "line": l.line}));
                }
            }
            if !dry_run && updated != content {
                std::fs::write(&abs, updated)?;
            }
        }
    }

    if !dry_run {
        let new_abs = root.join(&new_rel);
        if let Some(parent) = new_abs.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(root.join(&old_path), new_abs)?;
        vault::invalidate(root);
    }

    Ok(json!({
        "renamed_from": old_path,
        "renamed_to": new_rel,
        "dry_run": dry_run,
        "link_rewrites": changes,
    }))
}

// ---- convert_canvas ----

pub fn convert_canvas(canvas_path: &Path) -> Result<Value> {
    let json = std::fs::read_to_string(canvas_path)
        .map_err(|e| anyhow!("Cannot read {}: {}", canvas_path.display(), e))?;
    let canvas = super::canvas::parse_canvas(&json)?;
    let name = canvas_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Canvas");
    let markdown = super::canvas::canvas_to_markdown(&canvas, name);
    Ok(json!({
        "canvas": canvas_path.display().to_string(),
        "node_count": canvas.nodes.len(),
        "edge_count": canvas.edges.len(),
        "markdown": markdown,
    }))
}

// ---- extract_dataview_fields ----

pub fn extract_dataview_fields(root: &Path, note: Option<&str>, field: Option<&str>) -> Result<Value> {
    let idx = vault::get_index(root)?;
    let paths: Vec<String> = match note {
        Some(n) => vec![resolve_note_arg(&idx, n)?],
        None => {
            let mut p: Vec<String> = idx.notes.keys().cloned().collect();
            p.sort();
            p
        }
    };

    let want = field.map(|f| f.to_lowercase());
    let mut hits = Vec::new();
    for path in paths {
        let meta = &idx.notes[&path];

        // Frontmatter fields
        if let serde_yaml::Value::Mapping(map) = &meta.frontmatter {
            for (k, v) in map {
                let Some(key) = k.as_str() else { continue };
                if let Some(w) = &want {
                    if key.to_lowercase() != *w {
                        continue;
                    }
                }
                let value = serde_yaml::to_string(v).unwrap_or_default().trim().to_string();
                hits.push(json!({"note": path, "key": key, "value": value, "source": "frontmatter"}));
            }
        }

        // Inline `key:: value` fields
        let Ok(content) = std::fs::read_to_string(root.join(&path)) else { continue };
        let (_, body) = super::frontmatter::split(&content);
        for f in super::dataview::parse_inline_fields(body) {
            if let Some(w) = &want {
                if f.key.to_lowercase() != *w {
                    continue;
                }
            }
            hits.push(json!({"note": path, "key": f.key, "value": f.value, "line": f.line, "source": "inline"}));
        }
    }
    Ok(json!({"field_count": hits.len(), "fields": hits}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_vault")
    }

    #[test]
    fn vault_index_reports_broken_and_orphans() {
        let v = vault_index(&fixture(), true).unwrap();
        assert!(v["note_count"].as_u64().unwrap() >= 6);
        let broken: Vec<_> = v["broken_links"].as_array().unwrap().to_vec();
        assert!(broken.iter().any(|b| b["link"].as_str().unwrap().contains("Does Not Exist")));
        let orphans = v["orphans"].as_array().unwrap();
        assert!(orphans.iter().any(|o| o.as_str().unwrap().contains("Orphan")));
    }

    #[test]
    fn get_note_by_alias_with_embeds() {
        let v = get_note(&fixture(), "Alpha", false, 3).unwrap();
        assert_eq!(v["path"], "Note A.md");
        assert_eq!(v["frontmatter"]["title"], "Note A");
        assert!(v["tags"].as_array().unwrap().iter().any(|t| t == "project"));
    }

    #[test]
    fn resolve_link_heading_and_block() {
        let v = resolve_link(&fixture(), "[[Note B#Section One]]", Some("Note A.md")).unwrap();
        assert_eq!(v["resolution"]["status"], "Resolved");
        assert_eq!(v["heading_found"], true);

        let v = resolve_link(&fixture(), "Note B#^quote1", None).unwrap();
        assert!(v["block_line"].as_u64().is_some());
    }

    #[test]
    fn backlinks_include_alias_form() {
        let v = get_backlinks(&fixture(), "Note A").unwrap();
        let bl = v["backlinks"].as_array().unwrap();
        // Note B links via [[Note A]] and [[Alpha]]
        assert!(bl.iter().filter(|b| b["from"] == "Note B.md").count() >= 2);
    }

    #[test]
    fn search_modes() {
        let v = search(&fixture(), "status/active", "tag", 50).unwrap();
        assert!(v["hit_count"].as_u64().unwrap() >= 1);
        // nested prefix
        let v = search(&fixture(), "status", "tag", 50).unwrap();
        assert!(v["hit_count"].as_u64().unwrap() >= 1);
        let v = search(&fixture(), "beta", "alias", 50).unwrap();
        assert_eq!(v["hits"][0]["note"], "Note B.md");
        let v = search(&fixture(), "title=Note A", "field", 50).unwrap();
        assert_eq!(v["hits"][0]["note"], "Note A.md");
        let v = search(&fixture(), "quotable line", "text", 50).unwrap();
        assert_eq!(v["hits"][0]["note"], "Note B.md");
    }

    #[test]
    fn list_tasks_states_and_filter() {
        let v = list_tasks(&fixture(), None, Some("Note B")).unwrap();
        assert_eq!(v["task_count"].as_u64(), Some(5));
        assert_eq!(v["status_counts"]["done"].as_u64(), Some(1));

        let v = list_tasks(&fixture(), Some("open"), None).unwrap();
        assert!(v["tasks"].as_array().unwrap().iter().all(|t| t["task"]["status"] == "open"));
    }

    #[test]
    fn vault_config() {
        let v = get_vault_config(&fixture()).unwrap();
        assert_eq!(v["daily_notes_folder"], "daily");
        assert_eq!(v["attachment_folder"], "attachments");
    }

    /// Copy the fixture vault into a temp dir for write tests.
    fn scratch_vault(tag: &str) -> PathBuf {
        let dst = std::env::temp_dir().join(format!("mini_vault_test_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dst);
        copy_dir(&fixture(), &dst).unwrap();
        dst
    }

    fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let to = dst.join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &to)?;
            } else {
                std::fs::copy(entry.path(), to)?;
            }
        }
        Ok(())
    }

    #[test]
    fn rename_dry_run_then_real() {
        let vault = scratch_vault("rename");

        // Dry run: reports rewrites but changes nothing.
        let v = rename_note(&vault, "Note B", "Note Beta", true).unwrap();
        assert_eq!(v["renamed_to"], "Note Beta.md");
        let rewrites = v["link_rewrites"].as_array().unwrap();
        assert!(!rewrites.is_empty());
        assert!(vault.join("Note B.md").exists());

        // Real rename: file moves and inbound links rewritten with fragments intact.
        let v = rename_note(&vault, "Note B", "Note Beta", false).unwrap();
        assert!(!vault.join("Note B.md").exists());
        assert!(vault.join("Note Beta.md").exists());
        let a = std::fs::read_to_string(vault.join("Note A.md")).unwrap();
        assert!(a.contains("[[Note Beta]]"), "{}", a);
        assert!(a.contains("[[Note Beta|the second note]]"));
        assert!(a.contains("[[Note Beta#Section One]]"));
        assert!(a.contains("[[Note Beta#^quote1]]"));
        let _ = v;
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn create_from_template_and_daily() {
        let vault = scratch_vault("create");

        let v = create_note_from_template(&vault, Some("projects/New Idea"), Some("templates/Daily"), false).unwrap();
        assert_eq!(v["created"], "projects/New Idea.md");
        let content = std::fs::read_to_string(vault.join("projects/New Idea.md")).unwrap();
        assert!(content.starts_with("# New Idea"));
        assert!(!content.contains("{{date}}"));

        let v = create_note_from_template(&vault, None, None, true).unwrap();
        let created = v["created"].as_str().unwrap();
        assert!(created.starts_with("daily/"), "{}", created);
        assert!(vault.join(created).exists());

        // Refuses overwrite
        assert!(create_note_from_template(&vault, None, None, true).is_err());
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn canvas_and_dataview_tools() {
        let v = convert_canvas(&fixture().join("Board.canvas")).unwrap();
        assert_eq!(v["node_count"].as_u64(), Some(4));
        assert!(v["markdown"].as_str().unwrap().contains("# Canvas: Board"));

        let v = extract_dataview_fields(&fixture(), Some("Note B"), None).unwrap();
        let fields = v["fields"].as_array().unwrap();
        assert!(fields.iter().any(|f| f["key"] == "priority" && f["source"] == "inline"));
        assert!(fields.iter().any(|f| f["source"] == "frontmatter"));

        let v = extract_dataview_fields(&fixture(), None, Some("priority")).unwrap();
        assert!(v["fields"].as_array().unwrap().iter().all(|f| f["key"] == "priority"));
    }

    #[test]
    fn transclusion() {
        // Note A embeds img.png (attachment, untouched). Add heading embed via direct call:
        let idx = vault::get_index(&fixture()).unwrap();
        let out = transclude(&idx, &fixture(), "before ![[Note B#Section One]] after", "Note A.md", 3).unwrap();
        assert!(out.contains("quotable line"), "{}", out);
        assert!(out.contains("> [!quote]"));
    }
}
