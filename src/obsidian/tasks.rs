use serde::Serialize;

/// A checkbox task: `- [x] text`, with any single-char state, plus
/// Tasks-plugin emoji metadata (📅 due, ✅ done, ⏳ scheduled, 🛫 start, 🔁 recurrence).
#[derive(Debug, Clone, Serialize)]
pub struct Task {
    /// The raw state char inside the brackets: ' ', 'x', '/', '-', '>', ...
    pub state: char,
    /// Canonical status name for common states.
    pub status: String,
    pub text: String,
    pub tags: Vec<String>,
    pub due: Option<String>,
    pub done: Option<String>,
    pub scheduled: Option<String>,
    pub start: Option<String>,
    pub recurrence: Option<String>,
    /// 1-indexed line in the file.
    pub line: usize,
    /// Nesting depth (leading indentation / 2).
    pub indent: usize,
}

pub fn status_name(state: char) -> String {
    match state {
        ' ' => "open",
        'x' | 'X' => "done",
        '/' => "in_progress",
        '-' => "cancelled",
        '>' => "forwarded",
        '<' => "scheduled",
        '?' => "question",
        '!' => "important",
        _ => "other",
    }
    .to_string()
}

/// Parse all checkbox tasks in a document. Skips fenced code blocks.
pub fn parse_tasks(md: &str) -> Vec<Task> {
    let mut tasks = Vec::new();
    let mut in_fence = false;

    for (i, line) in md.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        // `- [s] `, `* [s] `, `+ [s] `, or numbered `1. [s] `
        let after_marker = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
            .or_else(|| {
                let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
                if digits > 0 {
                    trimmed[digits..].strip_prefix(". ")
                } else {
                    None
                }
            });
        let Some(rest) = after_marker else { continue };
        let mut chars = rest.chars();
        if chars.next() != Some('[') {
            continue;
        }
        let Some(state) = chars.next() else { continue };
        if chars.next() != Some(']') {
            continue;
        }
        let text_raw = chars.as_str().trim_start();
        if text_raw.is_empty() && state == ' ' && !rest.starts_with("[ ]") {
            continue;
        }

        let (text, due, done, scheduled, start, recurrence) = extract_metadata(text_raw);
        let tags = text_raw
            .split_whitespace()
            .filter_map(|w| w.strip_prefix('#'))
            .filter(|t| !t.is_empty() && t.chars().any(|c| c.is_alphabetic()))
            .map(|t| t.trim_end_matches(|c: char| !(c.is_alphanumeric() || c == '/' || c == '-' || c == '_')).to_string())
            .collect();

        let indent = (line.len() - trimmed.len()) / 2;
        tasks.push(Task {
            state,
            status: status_name(state),
            text,
            tags,
            due,
            done,
            scheduled,
            start,
            recurrence,
            line: i + 1,
            indent,
        });
    }
    tasks
}

/// Pull Tasks-plugin emoji metadata out of the task text.
#[allow(clippy::type_complexity)]
fn extract_metadata(
    text: &str,
) -> (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) {
    let mut due = None;
    let mut done = None;
    let mut scheduled = None;
    let mut start = None;
    let mut recurrence = None;

    let mut clean = String::new();
    let mut iter = text.split_whitespace().peekable();
    while let Some(word) = iter.next() {
        let mut grab_date = |slot: &mut Option<String>| {
            if let Some(next) = iter.peek() {
                if next.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    *slot = Some(iter.next().unwrap().to_string());
                }
            }
        };
        match word {
            "📅" | "🗓️" => grab_date(&mut due),
            "✅" => grab_date(&mut done),
            "⏳" => grab_date(&mut scheduled),
            "🛫" => grab_date(&mut start),
            "🔁" => {
                // recurrence is free text until the next emoji marker
                let mut parts = Vec::new();
                while let Some(next) = iter.peek() {
                    if ["📅", "🗓️", "✅", "⏳", "🛫", "🔁"].contains(next) {
                        break;
                    }
                    parts.push(iter.next().unwrap());
                }
                if !parts.is_empty() {
                    recurrence = Some(parts.join(" "));
                }
            }
            _ => {
                if !clean.is_empty() {
                    clean.push(' ');
                }
                clean.push_str(word);
            }
        }
    }
    (clean, due, done, scheduled, start, recurrence)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_states() {
        let md = "\
- [ ] open task #todo
- [x] done task ✅ 2026-07-01
- [/] halfway
- [-] dropped
- [>] moved 📅 2026-07-10
  - [ ] nested child
1. [ ] numbered task
";
        let tasks = parse_tasks(md);
        assert_eq!(tasks.len(), 7);
        assert_eq!(tasks[0].status, "open");
        assert_eq!(tasks[0].tags, vec!["todo"]);
        assert_eq!(tasks[1].status, "done");
        assert_eq!(tasks[1].done.as_deref(), Some("2026-07-01"));
        assert_eq!(tasks[2].status, "in_progress");
        assert_eq!(tasks[3].status, "cancelled");
        assert_eq!(tasks[4].status, "forwarded");
        assert_eq!(tasks[4].due.as_deref(), Some("2026-07-10"));
        assert_eq!(tasks[5].indent, 1);
        assert_eq!(tasks[6].line, 7);
    }

    #[test]
    fn skips_non_tasks_and_fences() {
        let md = "- plain bullet\n```\n- [ ] in fence\n```\n[x] no bullet";
        assert!(parse_tasks(md).is_empty());
    }
}
