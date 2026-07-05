use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

/// Render a Markdown document as styled ratatui Text. Pure function so it can
/// be unit-tested without a terminal.
pub fn markdown_to_text(md: &str) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_fence = false;

    for raw in md.lines() {
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

        // Callout header / blockquote
        if trimmed.starts_with('>') {
            let style = if trimmed.contains("[!") {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };
            lines.push(Line::from(Span::styled(raw.to_string(), style)));
            continue;
        }

        lines.push(render_inline(raw));
    }

    Text::from(lines)
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
}
