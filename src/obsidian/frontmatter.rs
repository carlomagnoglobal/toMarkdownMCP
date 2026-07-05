use anyhow::Result;
use serde_yaml::Value;

/// Split a document into (frontmatter_source, body). The frontmatter block is
/// a leading `---` line up to the next `---`/`...` line.
pub fn split(content: &str) -> (Option<&str>, &str) {
    let rest = match content.strip_prefix("---") {
        Some(r) if r.starts_with('\n') || r.starts_with("\r\n") => r,
        _ => return (None, content),
    };
    for (idx, line) in rest.match_indices('\n') {
        let after = &rest[idx + line.len()..];
        if after.starts_with("---") || after.starts_with("...") {
            let end_line = after.lines().next().unwrap_or("");
            if end_line.trim_end() == "---" || end_line.trim_end() == "..." {
                let fm = &rest[..idx];
                let body_start = idx + 1 + end_line.len();
                let body = after.get(end_line.len()..).unwrap_or("");
                let _ = body_start;
                let body = body.strip_prefix('\n').unwrap_or(body);
                return (Some(fm), body);
            }
        }
    }
    (None, content)
}

/// Parse the YAML frontmatter of a note. Returns Null when absent or invalid.
pub fn parse(content: &str) -> Value {
    match split(content) {
        (Some(src), _) => serde_yaml::from_str(src).unwrap_or(Value::Null),
        (None, _) => Value::Null,
    }
}

/// Extract a frontmatter field that may be a string, a YAML list, or a
/// comma-separated string — normalized to a Vec<String>. Used for
/// `aliases:` and `tags:`.
pub fn string_list(fm: &Value, key: &str) -> Vec<String> {
    let Some(v) = fm.get(key) else { return Vec::new() };
    match v {
        Value::String(s) => s
            .split(',')
            .map(|p| p.trim().trim_start_matches('#').to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        Value::Sequence(seq) => seq
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(s.trim().trim_start_matches('#').to_string()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .filter(|p| !p.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Serialize a frontmatter value back to a `---` block plus body.
pub fn render(fm: &Value, body: &str) -> Result<String> {
    if fm.is_null() {
        return Ok(body.to_string());
    }
    let yaml = serde_yaml::to_string(fm)?;
    Ok(format!("---\n{}---\n{}", yaml, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\ntitle: My Note\naliases:\n  - Alt Name\n  - Second\ntags: [a, b/c]\n---\n# Body\ntext\n";

    #[test]
    fn parses_lists_and_body() {
        let (fm_src, body) = split(NOTE);
        assert!(fm_src.is_some());
        assert!(body.starts_with("# Body"));

        let fm = parse(NOTE);
        assert_eq!(fm.get("title").and_then(|v| v.as_str()), Some("My Note"));
        assert_eq!(string_list(&fm, "aliases"), vec!["Alt Name", "Second"]);
        assert_eq!(string_list(&fm, "tags"), vec!["a", "b/c"]);
    }

    #[test]
    fn comma_string_tags() {
        let fm = parse("---\ntags: one, two\n---\nx");
        assert_eq!(string_list(&fm, "tags"), vec!["one", "two"]);
    }

    #[test]
    fn no_frontmatter() {
        assert_eq!(parse("# Just a doc"), Value::Null);
        let (fm, body) = split("# Just a doc");
        assert!(fm.is_none());
        assert_eq!(body, "# Just a doc");
        // A --- ruler later in the doc is not frontmatter
        assert_eq!(parse("text\n---\nmore"), Value::Null);
    }

    #[test]
    fn round_trip() {
        let fm = parse(NOTE);
        let (_, body) = split(NOTE);
        let out = render(&fm, body).unwrap();
        let fm2 = parse(&out);
        assert_eq!(string_list(&fm2, "aliases"), vec!["Alt Name", "Second"]);
        assert!(out.contains("# Body"));
    }
}
