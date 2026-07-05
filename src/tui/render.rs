use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use std::collections::HashMap;

/// Per-line callout styling, keyed by 0-indexed line number.
enum CalloutLine {
    Header { color: Color, label: String },
    Body { color: Color },
}

fn callout_style_for(kind: &str) -> (Color, &'static str) {
    match kind {
        "note" | "info" => (Color::Blue, "📝"),
        "tip" | "hint" | "success" | "check" | "done" => (Color::Green, "💡"),
        "warning" | "caution" | "attention" => (Color::Yellow, "⚠️"),
        "danger" | "error" | "bug" | "fail" | "failure" => (Color::Red, "🚫"),
        "important" => (Color::Magenta, "❗"),
        "question" | "help" | "faq" => (Color::Cyan, "❓"),
        "quote" | "cite" => (Color::Gray, "💬"),
        "example" => (Color::LightMagenta, "🧪"),
        _ => (Color::Yellow, "📌"),
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Map every line covered by a callout (header + body) to its styling.
fn build_callout_map(md: &str) -> HashMap<usize, CalloutLine> {
    let mut map = HashMap::new();
    for c in crate::obsidian::callout::parse_callouts(md) {
        let (color, icon) = callout_style_for(&c.kind);
        let label = match &c.title {
            Some(t) => format!("{} {}: {}", icon, capitalize(&c.kind), t),
            None => format!("{} {}", icon, capitalize(&c.kind)),
        };
        let header_idx = c.line - 1; // 0-indexed
        map.insert(header_idx, CalloutLine::Header { color, label });
        for j in 0..c.body.lines().count() {
            map.insert(header_idx + 1 + j, CalloutLine::Body { color });
        }
    }
    map
}

/// Render a Markdown document as styled ratatui Text. Pure function so it can
/// be unit-tested without a terminal.
pub fn markdown_to_text(md: &str) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_fence = false;
    let callouts = build_callout_map(md);

    for (i, raw) in md.lines().enumerate() {
        let trimmed = raw.trim_start();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            lines.push(Line::from(Span::styled(
                raw.to_string(),
                Style::default().fg(Color::DarkGray),
            )));
            continue;
        }
        if in_fence {
            lines.push(Line::from(Span::styled(
                raw.to_string(),
                Style::default().fg(Color::Green),
            )));
            continue;
        }

        // Headings
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && trimmed.chars().nth(hashes) == Some(' ') {
            let style = match hashes {
                1 => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                2 => Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD),
                _ => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            };
            lines.push(Line::from(Span::styled(trimmed.to_string(), style)));
            continue;
        }

        // Callouts (styled box: icon + kind + title, colored body bar)
        if let Some(info) = callouts.get(&i) {
            match info {
                CalloutLine::Header { color, label } => {
                    lines.push(Line::from(Span::styled(
                        label.clone(),
                        Style::default().fg(*color).add_modifier(Modifier::BOLD),
                    )));
                }
                CalloutLine::Body { color } => {
                    let body_text = trimmed.strip_prefix('>').unwrap_or(trimmed).trim_start();
                    lines.push(Line::from(vec![
                        Span::styled("┃ ", Style::default().fg(*color)),
                        Span::styled(body_text.to_string(), Style::default().fg(*color)),
                    ]));
                }
            }
            continue;
        }

        // Plain blockquote (not a callout)
        if trimmed.starts_with('>') {
            lines.push(Line::from(Span::styled(raw.to_string(), Style::default().fg(Color::Yellow))));
            continue;
        }

        lines.push(render_inline(raw));
    }

    Text::from(lines)
}

fn link_target_base(s: &str) -> &str {
    s.split(['|', '#']).next().unwrap_or(s)
}

fn is_image_ext(target: &str) -> bool {
    let lower = target.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".bmp", ".avif"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// Inline styling: **bold**, *italic*, `code`, [[wikilinks]], checkboxes.
fn render_inline(line: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    let flush = |buf: &mut String, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            spans.push(Span::raw(std::mem::take(buf)));
        }
    };

    // Checkbox prefix coloring
    let trimmed = line.trim_start();
    if trimmed.starts_with("- [") && trimmed.len() >= 5 && trimmed.as_bytes()[4] == b']' {
        let state = trimmed.as_bytes()[3] as char;
        let color = match state {
            ' ' => Color::White,
            'x' | 'X' => Color::Green,
            '/' => Color::Yellow,
            '-' => Color::DarkGray,
            _ => Color::Magenta,
        };
        return Line::from(Span::styled(line.to_string(), Style::default().fg(color)));
    }

    while i < chars.len() {
        // ![[embed]] — image embeds get a distinct placeholder, other
        // embeds (notes) render like normal wikilinks including the '!'.
        if chars[i] == '!' && i + 2 < chars.len() && chars[i + 1] == '[' && chars[i + 2] == '[' {
            if let Some(end) = find_seq(&chars, i + 3, "]]") {
                flush(&mut buf, &mut spans);
                let inner: String = chars[i + 3..end].iter().collect();
                if is_image_ext(link_target_base(&inner)) {
                    spans.push(Span::styled(
                        format!("🖼 {}", link_target_base(&inner)),
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC),
                    ));
                } else {
                    let whole: String = chars[i..end + 2].iter().collect();
                    spans.push(Span::styled(
                        whole,
                        Style::default().fg(Color::LightBlue).add_modifier(Modifier::UNDERLINED),
                    ));
                }
                i = end + 2;
                continue;
            }
        }
        // ![alt](url) — standard Markdown image syntax.
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some(alt_end_rel) = chars[i + 2..].iter().position(|&c| c == ']') {
                let alt_end = i + 2 + alt_end_rel;
                if chars.get(alt_end + 1) == Some(&'(') {
                    if let Some(paren_end_rel) = chars[alt_end + 2..].iter().position(|&c| c == ')') {
                        let paren_end = alt_end + 2 + paren_end_rel;
                        flush(&mut buf, &mut spans);
                        let alt: String = chars[i + 2..alt_end].iter().collect();
                        let url: String = chars[alt_end + 2..paren_end].iter().collect();
                        let label = if alt.is_empty() { url } else { alt };
                        spans.push(Span::styled(
                            format!("🖼 {}", label),
                            Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC),
                        ));
                        i = paren_end + 1;
                        continue;
                    }
                }
            }
        }
        // [[wikilink]]
        if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some(end) = find_seq(&chars, i + 2, "]]") {
                flush(&mut buf, &mut spans);
                let inner: String = chars[i..end + 2].iter().collect();
                spans.push(Span::styled(
                    inner,
                    Style::default().fg(Color::LightBlue).add_modifier(Modifier::UNDERLINED),
                ));
                i = end + 2;
                continue;
            }
        }
        // `code`
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '`') {
                flush(&mut buf, &mut spans);
                let inner: String = chars[i..i + 1 + end + 1].iter().collect();
                spans.push(Span::styled(inner, Style::default().fg(Color::Green)));
                i = i + 1 + end + 1;
                continue;
            }
        }
        // **bold**
        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(end) = find_seq(&chars, i + 2, "**") {
                flush(&mut buf, &mut spans);
                let inner: String = chars[i + 2..end].iter().collect();
                spans.push(Span::styled(inner, Style::default().add_modifier(Modifier::BOLD)));
                i = end + 2;
                continue;
            }
        }
        // *italic*
        if chars[i] == '*' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '*') {
                flush(&mut buf, &mut spans);
                let inner: String = chars[i + 1..i + 1 + end].iter().collect();
                spans.push(Span::styled(inner, Style::default().add_modifier(Modifier::ITALIC)));
                i = i + 1 + end + 1;
                continue;
            }
        }
        buf.push(chars[i]);
        i += 1;
    }
    flush(&mut buf, &mut spans);
    Line::from(spans)
}

fn find_seq(chars: &[char], from: usize, seq: &str) -> Option<usize> {
    let needle: Vec<char> = seq.chars().collect();
    (from..chars.len().saturating_sub(needle.len() - 1))
        .find(|&i| chars[i..i + needle.len()] == needle[..])
}

/// Word-wrap a styled line into display rows of at most `width` columns,
/// preserving span styles across the breaks. Guarantees every returned row
/// fits in `width`, so callers can scroll by display rows exactly.
pub fn wrap_styled_line(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthChar;

    let width = width.max(1);
    // Flatten the line into (char, style) pairs.
    let chars: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|s| s.content.chars().map(move |c| (c, s.style)))
        .collect();

    if chars.is_empty() {
        return vec![Line::from("")];
    }

    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut row: Vec<(char, Style)> = Vec::new();
    let mut row_width = 0usize;
    let mut last_space: Option<usize> = None; // index in `row` of the last space

    let mut flush =
        |row: &mut Vec<(char, Style)>, rows: &mut Vec<Line<'static>>| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut buf = String::new();
            let mut cur_style: Option<Style> = None;
            for &(c, style) in row.iter() {
                if Some(style) != cur_style {
                    if let Some(st) = cur_style {
                        if !buf.is_empty() {
                            spans.push(Span::styled(std::mem::take(&mut buf), st));
                        }
                    }
                    cur_style = Some(style);
                }
                buf.push(c);
            }
            if let Some(st) = cur_style {
                if !buf.is_empty() {
                    spans.push(Span::styled(buf, st));
                }
            }
            rows.push(Line::from(spans));
            row.clear();
        };

    for (c, style) in chars {
        let cw = c.width().unwrap_or(0);
        if row_width + cw > width {
            match last_space {
                // Break at the last space: it stays on the current row,
                // the word after it moves to the next row.
                Some(sp) if sp + 1 < row.len() => {
                    let carry: Vec<(char, Style)> = row.split_off(sp + 1);
                    flush(&mut row, &mut rows);
                    row = carry;
                    row_width = row.iter().map(|(c, _)| c.width().unwrap_or(0)).sum();
                }
                _ => {
                    // No space to break at (single long word): hard break.
                    flush(&mut row, &mut rows);
                    row_width = 0;
                }
            }
            last_space = None;
            // Drop a space that would land at the start of the new row.
            if c == ' ' && row.is_empty() {
                continue;
            }
        }
        if c == ' ' {
            last_space = Some(row.len());
        }
        row.push((c, style));
        row_width += cw;
    }
    if !row.is_empty() || rows.is_empty() {
        flush(&mut row, &mut rows);
    }
    rows
}

/// All wikilink raw strings in a line, for cursor-based link following.
pub fn wikilinks_in_line(line: &str) -> Vec<String> {
    crate::obsidian::wikilink::parse_wikilinks(line)
        .into_iter()
        .map(|l| l.target)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_is_bold() {
        let text = markdown_to_text("# Title\nplain");
        assert_eq!(text.lines.len(), 2);
        let first = &text.lines[0].spans[0];
        assert!(first.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(first.content.as_ref(), "# Title");
    }

    #[test]
    fn wikilink_span_underlined() {
        let text = markdown_to_text("see [[Note A]] now");
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content.as_ref() == "[[Note A]]"
            && s.style.add_modifier.contains(Modifier::UNDERLINED)));
    }

    #[test]
    fn fence_toggles() {
        let text = markdown_to_text("```\n# not a heading\n```");
        let mid = &text.lines[1].spans[0];
        assert!(!mid.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn checkbox_colors() {
        let text = markdown_to_text("- [x] done");
        assert_eq!(text.lines[0].spans[0].style.fg, Some(Color::Green));
    }

    #[test]
    fn links_in_line() {
        assert_eq!(wikilinks_in_line("a [[X]] b [[Y|y]]"), vec!["X", "Y"]);
    }

    #[test]
    fn callout_header_and_body_styled() {
        let md = "> [!warning] Careful\n> body line";
        let text = markdown_to_text(md);
        let header = &text.lines[0].spans[0];
        assert!(header.content.contains("Warning: Careful"));
        assert!(header.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(header.style.fg, Some(Color::Yellow));

        let body = &text.lines[1];
        assert_eq!(body.spans[0].content.as_ref(), "┃ ");
        assert_eq!(body.spans[1].content.as_ref(), "body line");
        assert_eq!(body.spans[1].style.fg, Some(Color::Yellow));
    }

    #[test]
    fn plain_blockquote_not_treated_as_callout() {
        let text = markdown_to_text("> just a quote");
        assert_eq!(text.lines[0].spans[0].content.as_ref(), "> just a quote");
    }

    #[test]
    fn image_embed_placeholder() {
        let text = markdown_to_text("before ![[photo.png]] after");
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content.as_ref() == "🖼 photo.png"
            && s.style.fg == Some(Color::Magenta)));
    }

    #[test]
    fn non_image_embed_keeps_wikilink_style() {
        let text = markdown_to_text("![[Some Note]]");
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content.as_ref() == "![[Some Note]]"
            && s.style.add_modifier.contains(Modifier::UNDERLINED)));
    }

    #[test]
    fn markdown_image_placeholder() {
        let text = markdown_to_text("see ![a cat](cat.jpg) here");
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content.as_ref() == "🖼 a cat"));
    }

    fn row_text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn wrap_breaks_at_word_boundaries_within_width() {
        let line = Line::from("the quick brown fox jumps over the lazy dog");
        let rows = wrap_styled_line(&line, 16);
        assert!(rows.len() > 1);
        for r in &rows {
            let t = row_text(r);
            assert!(t.chars().count() <= 16, "row too wide: {:?}", t);
            assert!(!t.starts_with(' '), "row starts with space: {:?}", t);
        }
        let joined: String = rows.iter().map(row_text).collect::<Vec<_>>().join(" ");
        assert!(joined.split_whitespace().eq("the quick brown fox jumps over the lazy dog".split_whitespace()));
    }

    #[test]
    fn wrap_preserves_span_styles_across_break() {
        let line = Line::from(vec![
            Span::raw("plain start "),
            Span::styled("bold segment that will wrap", Style::default().add_modifier(Modifier::BOLD)),
        ]);
        let rows = wrap_styled_line(&line, 18);
        assert!(rows.len() >= 2);
        // Bold text on the continuation row keeps its style.
        let last = rows.last().unwrap();
        assert!(last.spans.iter().all(|s| s.style.add_modifier.contains(Modifier::BOLD)));
    }

    #[test]
    fn wrap_hard_breaks_single_long_word_and_handles_empty() {
        let rows = wrap_styled_line(&Line::from("abcdefghij"), 4);
        assert_eq!(rows.iter().map(row_text).collect::<Vec<_>>(), vec!["abcd", "efgh", "ij"]);

        let rows = wrap_styled_line(&Line::from(""), 10);
        assert_eq!(rows.len(), 1);

        // Short line: single row, unchanged.
        let rows = wrap_styled_line(&Line::from("short"), 40);
        assert_eq!(rows.len(), 1);
        assert_eq!(row_text(&rows[0]), "short");
    }
}
