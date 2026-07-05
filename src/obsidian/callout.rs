use serde::Serialize;

/// An Obsidian callout: `> [!type] Optional Title` followed by `> ` body lines.
#[derive(Debug, Clone, Serialize)]
pub struct Callout {
    pub kind: String,
    pub title: Option<String>,
    /// `-` = folded by default, `+` = expanded, None = static.
    pub fold: Option<char>,
    pub body: String,
    /// 1-indexed line of the `[!type]` marker.
    pub line: usize,
}

/// Parse all callouts (top-level; nested callouts stay in the parent body).
pub fn parse_callouts(md: &str) -> Vec<Callout> {
    let mut callouts = Vec::new();
    let lines: Vec<&str> = md.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if let Some(header) = parse_header(lines[i]) {
            let (kind, title, fold) = header;
            let mut body = Vec::new();
            let mut j = i + 1;
            while j < lines.len() {
                let t = lines[j].trim_start();
                if let Some(rest) = t.strip_prefix('>') {
                    body.push(rest.strip_prefix(' ').unwrap_or(rest));
                    j += 1;
                } else {
                    break;
                }
            }
            callouts.push(Callout { kind, title, fold, body: body.join("\n"), line: i + 1 });
            i = j;
        } else {
            i += 1;
        }
    }
    callouts
}

fn parse_header(line: &str) -> Option<(String, Option<String>, Option<char>)> {
    let t = line.trim_start().strip_prefix('>')?.trim_start();
    let rest = t.strip_prefix("[!")?;
    let close = rest.find(']')?;
    let mut kind = rest[..close].to_string();
    let mut fold = None;
    if let Some(f) = kind.strip_suffix('-') {
        // `[!tip]-` puts the fold marker after the bracket, but tolerate inside too
        kind = f.to_string();
        fold = Some('-');
    }
    let mut after = rest[close + 1..].trim_start();
    if let Some(a) = after.strip_prefix('-') {
        fold = Some('-');
        after = a.trim_start();
    } else if let Some(a) = after.strip_prefix('+') {
        fold = Some('+');
        after = a.trim_start();
    }
    if kind.is_empty() || !kind.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return None;
    }
    let title = if after.is_empty() { None } else { Some(after.to_string()) };
    Some((kind.to_lowercase(), title, fold))
}

/// Render a callout as plain Markdown (blockquote with a bold label).
pub fn callout_to_markdown(c: &Callout) -> String {
    let label = match &c.title {
        Some(t) => format!("**{}: {}**", capitalize(&c.kind), t),
        None => format!("**{}**", capitalize(&c.kind)),
    };
    let mut out = format!("> {}\n", label);
    for line in c.body.lines() {
        out.push_str("> ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_forms() {
        let md = "\
> [!note]
> A plain note callout.

> [!warning] Custom Title
> Be careful.
> Second line.

> [!tip]- Folded tip
> Hidden.

> regular blockquote
";
        let cs = parse_callouts(md);
        assert_eq!(cs.len(), 3);
        assert_eq!(cs[0].kind, "note");
        assert!(cs[0].title.is_none());
        assert_eq!(cs[0].body, "A plain note callout.");
        assert_eq!(cs[1].title.as_deref(), Some("Custom Title"));
        assert!(cs[1].body.contains("Second line."));
        assert_eq!(cs[2].fold, Some('-'));
        assert_eq!(cs[2].title.as_deref(), Some("Folded tip"));
    }

    #[test]
    fn renders_markdown() {
        let cs = parse_callouts("> [!warning] Careful\n> body");
        let md = callout_to_markdown(&cs[0]);
        assert!(md.contains("**Warning: Careful**"));
        assert!(md.contains("> body"));
    }
}
