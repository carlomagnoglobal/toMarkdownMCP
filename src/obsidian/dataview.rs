use serde::Serialize;

/// A Dataview inline field: `key:: value` on its own line, `[key:: value]`
/// or `(key:: value)` inline.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InlineField {
    pub key: String,
    pub value: String,
    /// 1-indexed line.
    pub line: usize,
}

/// Extract Dataview inline fields from a note body. Skips code blocks.
pub fn parse_inline_fields(md: &str) -> Vec<InlineField> {
    let mut fields = Vec::new();
    let mut in_fence = false;

    for (i, line) in md.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        // Bracketed inline forms: [key:: value] and (key:: value)
        for (open, close) in [('[', ']'), ('(', ')')] {
            let mut rest = trimmed;
            while let Some(start) = rest.find(open) {
                let Some(end_rel) = rest[start..].find(close) else { break };
                let inner = &rest[start + 1..start + end_rel];
                if let Some(f) = split_field(inner, i + 1) {
                    fields.push(f);
                }
                rest = &rest[start + end_rel + 1..];
            }
        }

        // Whole-line form: `key:: value` (also list items `- key:: value`)
        let candidate = trimmed.trim_start_matches("- ").trim_start_matches("* ");
        if !candidate.starts_with('[') && !candidate.starts_with('(') {
            if let Some(f) = split_field(candidate, i + 1) {
                fields.push(f);
            }
        }
    }
    fields
}

fn split_field(s: &str, line: usize) -> Option<InlineField> {
    let idx = s.find("::")?;
    let key = s[..idx].trim();
    let value = s[idx + 2..].trim();
    // Dataview keys are word-ish: letters/numbers/space/dash/underscore.
    if key.is_empty()
        || value.is_empty()
        || key.len() > 60
        || !key.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
    {
        return None;
    }
    Some(InlineField { key: key.to_string(), value: value.to_string(), line })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_line_and_bracketed() {
        let md = "\
priority:: high
- effort:: 3 days
Text with [status:: active] inline and (owner:: elis).
```
fenced:: skipped
```
url:: https://example.com/a::b
";
        let fields = parse_inline_fields(md);
        let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
        assert!(keys.contains(&"priority"));
        assert!(keys.contains(&"effort"));
        assert!(keys.contains(&"status"));
        assert!(keys.contains(&"owner"));
        assert!(!keys.contains(&"fenced"));
        // :: inside value preserved
        let url = fields.iter().find(|f| f.key == "url").unwrap();
        assert_eq!(url.value, "https://example.com/a::b");
    }

    #[test]
    fn rejects_non_fields() {
        assert!(parse_inline_fields("just text").is_empty());
        assert!(parse_inline_fields("a URL https://x.com/path is not field").is_empty());
    }
}
