use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use std::collections::HashMap;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

/// All colors used by the viewer, so themes can be swapped wholesale.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub h1: Color,
    pub h2: Color,
    pub h3: Color,
    pub rule: Color,
    pub text_dim: Color,
    pub bullet: Color,
    pub code_fg: Color,
    pub code_bg: Color,
    pub link: Color,
    pub quote: Color,
    pub fm_key: Color,
    pub fm_border: Color,
    pub table_border: Color,
    pub cursor_bg: Color,
    pub match_bg: Color,
    pub match_fg: Color,
    pub image: Color,
    pub border_focused: Color,
    pub border_unfocused: Color,
    pub tree_dir: Color,
    pub status_fg: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            name: "dark",
            h1: Color::Cyan,
            h2: Color::LightCyan,
            h3: Color::Blue,
            rule: Color::DarkGray,
            text_dim: Color::DarkGray,
            bullet: Color::Cyan,
            code_fg: Color::Green,
            code_bg: Color::Rgb(40, 42, 52),
            link: Color::LightBlue,
            quote: Color::Yellow,
            fm_key: Color::Gray,
            fm_border: Color::DarkGray,
            table_border: Color::DarkGray,
            cursor_bg: Color::Rgb(40, 40, 60),
            match_bg: Color::Yellow,
            match_fg: Color::Black,
            image: Color::Magenta,
            border_focused: Color::Cyan,
            border_unfocused: Color::DarkGray,
            tree_dir: Color::Yellow,
            status_fg: Color::DarkGray,
        }
    }

    pub fn light() -> Self {
        Theme {
            name: "light",
            h1: Color::Blue,
            h2: Color::Blue,
            h3: Color::Magenta,
            rule: Color::Gray,
            text_dim: Color::Gray,
            bullet: Color::Blue,
            code_fg: Color::Rgb(0, 100, 0),
            code_bg: Color::Rgb(232, 232, 236),
            link: Color::Blue,
            quote: Color::Rgb(150, 90, 0),
            fm_key: Color::DarkGray,
            fm_border: Color::Gray,
            table_border: Color::Gray,
            cursor_bg: Color::Rgb(215, 225, 245),
            match_bg: Color::Yellow,
            match_fg: Color::Black,
            image: Color::Magenta,
            border_focused: Color::Blue,
            border_unfocused: Color::Gray,
            tree_dir: Color::Rgb(150, 90, 0),
            status_fg: Color::Gray,
        }
    }

    pub fn by_index(i: usize) -> Self {
        if i % 2 == 0 { Theme::dark() } else { Theme::light() }
    }
}

// ---------------------------------------------------------------------------
// Callouts (colors + icons)
// ---------------------------------------------------------------------------

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

fn build_callout_map(md: &str) -> HashMap<usize, CalloutLine> {
    let mut map = HashMap::new();
    for c in crate::obsidian::callout::parse_callouts(md) {
        let (color, icon) = callout_style_for(&c.kind);
        let label = match &c.title {
            Some(t) => format!("{} {}: {}", icon, capitalize(&c.kind), t),
            None => format!("{} {}", icon, capitalize(&c.kind)),
        };
        let header_idx = c.line - 1;
        map.insert(header_idx, CalloutLine::Header { color, label });
        for j in 0..c.body.lines().count() {
            map.insert(header_idx + 1 + j, CalloutLine::Body { color });
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum TableRole {
    Header,
    Separator,
    Row,
}

struct TableInfo {
    role: TableRole,
    /// Shared column widths for the whole block.
    widths: std::rc::Rc<Vec<usize>>,
}

fn split_table_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

/// Display width of a table cell after inline rendering (markers stripped).
fn rendered_cell_width(cell: &str) -> usize {
    render_inline_spans(cell, &Theme::dark(), Style::default())
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
        .sum()
}

fn is_table_separator(line: &str) -> bool {
    let t = line.trim();
    if !t.contains('-') || !t.contains('|') && !t.starts_with('-') {
        return false;
    }
    !t.is_empty()
        && t.chars().all(|c| matches!(c, '-' | '|' | ':' | ' '))
        && t.contains('-')
        && t.contains('|')
}

/// Detect pipe-table blocks (header, separator, rows) outside code fences.
/// Returns line index -> table info.
fn build_table_map(lines: &[&str]) -> HashMap<usize, TableInfo> {
    let mut map = HashMap::new();
    let mut in_fence = false;
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            i += 1;
            continue;
        }
        if !in_fence
            && lines[i].contains('|')
            && i + 1 < lines.len()
            && is_table_separator(lines[i + 1])
        {
            // Collect the block.
            let start = i;
            let mut end = i + 2;
            while end < lines.len() && lines[end].contains('|') && !lines[end].trim().is_empty() {
                end += 1;
            }
            // Column widths across all rows (excluding separator), measured
            // on the *rendered* cell text (markdown markers stripped).
            let mut widths: Vec<usize> = Vec::new();
            for (j, line) in lines[start..end].iter().enumerate() {
                if j == 1 {
                    continue;
                }
                for (c, cell) in split_table_cells(line).iter().enumerate() {
                    let w = rendered_cell_width(cell);
                    if c >= widths.len() {
                        widths.push(w);
                    } else if w > widths[c] {
                        widths[c] = w;
                    }
                }
            }
            let widths = std::rc::Rc::new(widths);
            for (j, idx) in (start..end).enumerate() {
                let role = match j {
                    0 => TableRole::Header,
                    1 => TableRole::Separator,
                    _ => TableRole::Row,
                };
                map.insert(idx, TableInfo { role, widths: std::rc::Rc::clone(&widths) });
            }
            i = end;
            continue;
        }
        i += 1;
    }
    map
}

fn render_table_line(line: &str, info: &TableInfo, theme: &Theme) -> Line<'static> {
    let border = Style::default().fg(theme.table_border);
    match info.role {
        TableRole::Separator => {
            let mut s = String::from("├");
            for (i, w) in info.widths.iter().enumerate() {
                s.push_str(&"─".repeat(w + 2));
                s.push(if i + 1 == info.widths.len() { '┤' } else { '┼' });
            }
            Line::from(Span::styled(s, border))
        }
        role => {
            let cells = split_table_cells(line);
            let mut spans: Vec<Span<'static>> = vec![Span::styled("│", border)];
            for (i, w) in info.widths.iter().enumerate() {
                let cell = cells.get(i).cloned().unwrap_or_default();
                let base = if role == TableRole::Header {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let rendered = render_inline_spans(&cell, theme, base);
                let pad = w.saturating_sub(
                    rendered.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum(),
                );
                spans.push(Span::styled(" ", base));
                spans.extend(rendered);
                spans.push(Span::styled(" ".repeat(pad + 1), base));
                spans.push(Span::styled("│", border));
            }
            Line::from(spans)
        }
    }
}

// ---------------------------------------------------------------------------
// Syntax highlighting (syntect)
// ---------------------------------------------------------------------------

static SYNTECT: once_cell::sync::Lazy<(syntect::parsing::SyntaxSet, syntect::highlighting::ThemeSet)> =
    once_cell::sync::Lazy::new(|| {
        (
            syntect::parsing::SyntaxSet::load_defaults_nonewlines(),
            syntect::highlighting::ThemeSet::load_defaults(),
        )
    });

fn syntect_theme_for(theme: &Theme) -> &'static syntect::highlighting::Theme {
    let (_, themes) = &*SYNTECT;
    let name = if theme.name == "light" { "InspiredGitHub" } else { "base16-eighties.dark" };
    themes.themes.get(name).unwrap_or_else(|| themes.themes.values().next().unwrap())
}

/// Streaming per-fence highlighter state.
struct FenceHighlighter {
    inner: Option<syntect::easy::HighlightLines<'static>>,
}

impl FenceHighlighter {
    fn new(lang: &str, theme: &Theme) -> Self {
        let (syntaxes, _) = &*SYNTECT;
        let syntax = if lang.is_empty() {
            None
        } else {
            syntaxes
                .find_syntax_by_token(lang)
                .or_else(|| syntaxes.find_syntax_by_extension(lang))
        };
        FenceHighlighter {
            inner: syntax
                .map(|s| syntect::easy::HighlightLines::new(s, syntect_theme_for(theme))),
        }
    }

    /// Highlight one code line into spans (bg forced to the theme's code_bg).
    fn line(&mut self, text: &str, theme: &Theme) -> Vec<Span<'static>> {
        let (syntaxes, _) = &*SYNTECT;
        match &mut self.inner {
            Some(hl) => match hl.highlight_line(text, syntaxes) {
                Ok(regions) => regions
                    .into_iter()
                    .map(|(st, seg)| {
                        Span::styled(
                            seg.to_string(),
                            Style::default()
                                .fg(Color::Rgb(st.foreground.r, st.foreground.g, st.foreground.b))
                                .bg(theme.code_bg),
                        )
                    })
                    .collect(),
                Err(_) => vec![Span::styled(
                    text.to_string(),
                    Style::default().fg(theme.code_fg).bg(theme.code_bg),
                )],
            },
            None => vec![Span::styled(
                text.to_string(),
                Style::default().fg(theme.code_fg).bg(theme.code_bg),
            )],
        }
    }
}

// ---------------------------------------------------------------------------
// Styled document (block-level pass)
// ---------------------------------------------------------------------------

/// One display element per entry, each mapped back to a logical content line.
pub struct StyledDoc {
    pub lines: Vec<Line<'static>>,
    /// Logical (content) line index for each styled line.
    pub logical: Vec<usize>,
    /// Hanging indent applied to wrapped continuation rows.
    pub hang: Vec<usize>,
    /// Lines that must not be wrapped (tables, rules) — truncated instead.
    pub no_wrap: Vec<bool>,
}

/// Setext headings: a text line followed by `===` (H1) or `---` (H2).
/// Maps both the text line and the underline line to the heading level.
#[derive(Clone, Copy, PartialEq)]
enum SetextRole {
    Text(u8),
    Underline(u8),
}

fn build_setext_map(lines: &[&str]) -> HashMap<usize, SetextRole> {
    let mut map = HashMap::new();
    let mut in_fence = false;
    for i in 0..lines.len().saturating_sub(1) {
        let t = lines[i].trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let text = lines[i].trim();
        let under = lines[i + 1].trim();
        if text.is_empty()
            || text.starts_with('#')
            || text.starts_with('>')
            || text.starts_with('|')
            || text.starts_with("- ")
        {
            continue;
        }
        let level = if under.len() >= 2 && under.chars().all(|c| c == '=') {
            Some(1)
        } else if under.len() >= 2 && under.chars().all(|c| c == '-') {
            Some(2)
        } else {
            None
        };
        if let Some(l) = level {
            map.insert(i, SetextRole::Text(l));
            map.insert(i + 1, SetextRole::Underline(l));
        }
    }
    map
}

fn is_hr(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3
        && (t.chars().all(|c| c == '-' || c == ' ')
            || t.chars().all(|c| c == '*' || c == ' ')
            || t.chars().all(|c| c == '_' || c == ' '))
        && t.chars().filter(|c| !c.is_whitespace()).count() >= 3
}

/// Style every logical line of a document. `width` is the pane inner width
/// (used for rules and box borders).
pub fn style_document(content: &str, raw: bool, width: usize, theme: &Theme) -> StyledDoc {
    let width = width.max(10);
    let mut doc = StyledDoc {
        lines: Vec::new(),
        logical: Vec::new(),
        hang: Vec::new(),
        no_wrap: Vec::new(),
    };
    let push = |doc: &mut StyledDoc, line: Line<'static>, logical: usize, hang: usize, no_wrap: bool| {
        doc.lines.push(line);
        doc.logical.push(logical);
        doc.hang.push(hang);
        doc.no_wrap.push(no_wrap);
    };

    if raw {
        for (i, l) in content.lines().enumerate() {
            push(&mut doc, Line::from(l.to_string()), i, 0, false);
        }
        if doc.lines.is_empty() {
            push(&mut doc, Line::from(""), 0, 0, false);
        }
        return doc;
    }

    let lines: Vec<&str> = content.lines().collect();
    let callouts = build_callout_map(content);
    let tables = build_table_map(&lines);
    let setext = build_setext_map(&lines);

    // Frontmatter range: leading `---` ... `---`/`...`
    let fm_end: Option<usize> = if lines.first().map(|l| l.trim_end() == "---").unwrap_or(false) {
        lines[1..]
            .iter()
            .position(|l| l.trim_end() == "---" || l.trim_end() == "...")
            .map(|p| p + 1)
    } else {
        None
    };

    let dim = Style::default().fg(theme.text_dim);
    let mut in_fence = false;
    let mut fence_hl: Option<FenceHighlighter> = None;

    for (i, rawline) in lines.iter().enumerate() {
        let trimmed = rawline.trim_start();

        // Frontmatter box
        if let Some(end) = fm_end {
            if i == 0 {
                let label = " Properties ";
                let fill = width.saturating_sub(2 + label.chars().count());
                push(
                    &mut doc,
                    Line::from(Span::styled(
                        format!("╭─{}{}", label, "─".repeat(fill)),
                        Style::default().fg(theme.fm_border),
                    )),
                    i,
                    0,
                    true,
                );
                continue;
            }
            if i == end {
                push(
                    &mut doc,
                    Line::from(Span::styled(
                        format!("╰{}", "─".repeat(width.saturating_sub(1))),
                        Style::default().fg(theme.fm_border),
                    )),
                    i,
                    0,
                    true,
                );
                continue;
            }
            if i < end {
                let line = match rawline.split_once(':') {
                    Some((k, v)) => Line::from(vec![
                        Span::styled("│ ", Style::default().fg(theme.fm_border)),
                        Span::styled(
                            format!("{}:", k.trim_end()),
                            Style::default().fg(theme.fm_key).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(v.to_string(), dim),
                    ]),
                    None => Line::from(vec![
                        Span::styled("│ ", Style::default().fg(theme.fm_border)),
                        Span::styled(rawline.to_string(), dim),
                    ]),
                };
                push(&mut doc, line, i, 4, false);
                continue;
            }
        }

        // Code fences
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if !in_fence {
                let lang = trimmed.trim_start_matches(['`', '~']).trim();
                fence_hl = Some(FenceHighlighter::new(lang, theme));
                let label = if lang.is_empty() { String::new() } else { format!(" {} ", lang) };
                let fill = width.saturating_sub(3 + label.chars().count());
                push(
                    &mut doc,
                    Line::from(Span::styled(
                        format!("╭──{}{}", label, "─".repeat(fill)),
                        Style::default().fg(theme.rule),
                    )),
                    i,
                    0,
                    true,
                );
            } else {
                fence_hl = None;
                push(
                    &mut doc,
                    Line::from(Span::styled(
                        format!("╰{}", "─".repeat(width.saturating_sub(1))),
                        Style::default().fg(theme.rule),
                    )),
                    i,
                    0,
                    true,
                );
            }
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            let mut spans = vec![Span::styled("│ ", Style::default().fg(theme.rule))];
            match &mut fence_hl {
                Some(hl) => spans.extend(hl.line(rawline, theme)),
                None => spans.push(Span::styled(
                    rawline.to_string(),
                    Style::default().fg(theme.code_fg).bg(theme.code_bg),
                )),
            }
            push(&mut doc, Line::from(spans), i, 2, false);
            continue;
        }

        // Tables
        if let Some(info) = tables.get(&i) {
            push(&mut doc, render_table_line(rawline, info, theme), i, 0, true);
            continue;
        }

        // Setext headings (text + ===/--- underline), as emitted by html2md.
        if let Some(role) = setext.get(&i) {
            match role {
                SetextRole::Text(level) => {
                    let (style, _) = heading_style(*level, theme);
                    push(
                        &mut doc,
                        Line::from(Span::styled(trimmed.trim().to_string(), style)),
                        i,
                        0,
                        false,
                    );
                }
                SetextRole::Underline(level) => {
                    let color = if *level == 1 { theme.h1 } else { theme.rule };
                    let text_w = lines[i - 1].trim().len().min(width).max(3);
                    push(
                        &mut doc,
                        Line::from(Span::styled("─".repeat(text_w), Style::default().fg(color))),
                        i,
                        0,
                        true,
                    );
                }
            }
            continue;
        }

        // Headings: strip the #, style by level, underline H1/H2.
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && trimmed.chars().nth(hashes) == Some(' ') {
            let text = trimmed[hashes + 1..].trim().to_string();
            let (style, underline) = heading_style(hashes as u8, theme);
            push(&mut doc, Line::from(Span::styled(text.clone(), style)), i, 0, false);
            if let Some(color) = underline {
                let w = UnicodeWidthStr::width(text.as_str()).min(width).max(3);
                push(
                    &mut doc,
                    Line::from(Span::styled("─".repeat(w), Style::default().fg(color))),
                    i,
                    0,
                    true,
                );
            }
            continue;
        }

        // Horizontal rule
        if is_hr(rawline) && fm_end.map(|e| i > e).unwrap_or(true) {
            push(
                &mut doc,
                Line::from(Span::styled("─".repeat(width), Style::default().fg(theme.rule))),
                i,
                0,
                true,
            );
            continue;
        }

        // Callouts
        if let Some(info) = callouts.get(&i) {
            match info {
                CalloutLine::Header { color, label } => {
                    push(
                        &mut doc,
                        Line::from(Span::styled(
                            label.clone(),
                            Style::default().fg(*color).add_modifier(Modifier::BOLD),
                        )),
                        i,
                        0,
                        false,
                    );
                }
                CalloutLine::Body { color } => {
                    let body = trimmed.strip_prefix('>').unwrap_or(trimmed).trim_start();
                    push(
                        &mut doc,
                        Line::from(vec![
                            Span::styled("┃ ", Style::default().fg(*color)),
                            Span::styled(body.to_string(), Style::default().fg(*color)),
                        ]),
                        i,
                        2,
                        false,
                    );
                }
            }
            continue;
        }

        // Plain blockquote
        if let Some(q) = trimmed.strip_prefix('>') {
            push(
                &mut doc,
                Line::from(vec![
                    Span::styled("▌ ", Style::default().fg(theme.quote)),
                    Span::styled(
                        q.trim_start().to_string(),
                        Style::default().fg(theme.quote).add_modifier(Modifier::ITALIC),
                    ),
                ]),
                i,
                2,
                false,
            );
            continue;
        }

        // Everything else: inline rendering (lists, checkboxes, links, ...)
        let indent_width = rawline.len() - trimmed.len();
        let (line, hang) = render_inline_line(rawline, trimmed, indent_width, theme);
        push(&mut doc, line, i, hang, false);
    }

    if doc.lines.is_empty() {
        push(&mut doc, Line::from(""), 0, 0, false);
    }
    doc
}

// ---------------------------------------------------------------------------
// Inline rendering
// ---------------------------------------------------------------------------

/// Render a normal text line: list bullets, checkboxes, then inline spans.
/// Returns the line plus its hanging indent for wrapped rows.
fn render_inline_line(_raw: &str, trimmed: &str, indent: usize, theme: &Theme) -> (Line<'static>, usize) {
    // Checkbox task lines: color by state, keep the whole line.
    if trimmed.starts_with("- [") && trimmed.len() >= 5 && trimmed.as_bytes()[4] == b']' {
        let state = trimmed.as_bytes()[3] as char;
        let color = match state {
            ' ' => Color::White,
            'x' | 'X' => Color::Green,
            '/' => Color::Yellow,
            '-' => Color::DarkGray,
            _ => Color::Magenta,
        };
        let mark = match state {
            ' ' => "☐",
            'x' | 'X' => "☑",
            _ => "◪",
        };
        let rest = &trimmed[5..];
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled(format!("{} ", mark), Style::default().fg(color)),
        ];
        spans.extend(render_inline_spans(rest.trim_start(), theme, Style::default().fg(color)));
        return (Line::from(spans), indent + 2);
    }

    // Bullet lists
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            let mut spans = vec![
                Span::raw(" ".repeat(indent)),
                Span::styled("• ", Style::default().fg(theme.bullet)),
            ];
            spans.extend(render_inline_spans(rest, theme, Style::default()));
            return (Line::from(spans), indent + 2);
        }
    }

    // Numbered lists
    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 3 {
        if let Some(rest) = trimmed[digits..].strip_prefix(". ") {
            let marker = &trimmed[..digits + 2];
            let mut spans = vec![
                Span::raw(" ".repeat(indent)),
                Span::styled(marker.to_string(), Style::default().add_modifier(Modifier::BOLD)),
            ];
            spans.extend(render_inline_spans(rest, theme, Style::default()));
            return (Line::from(spans), indent + digits + 2);
        }
    }

    let mut spans = Vec::new();
    if indent > 0 {
        spans.push(Span::raw(" ".repeat(indent)));
    }
    spans.extend(render_inline_spans(trimmed, theme, Style::default()));
    (Line::from(spans), 0)
}

/// Inline spans: **bold**, *italic*, __bold__, _italic_, ~~strike~~,
/// `code`, [[wikilinks]], [text](url), images.
fn render_inline_spans(text: &str, theme: &Theme, base: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    let flush = |buf: &mut String, spans: &mut Vec<Span<'static>>, base: Style| {
        if !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(buf), base));
        }
    };

    while i < chars.len() {
        // ![[embed]] — image or note embed
        if chars[i] == '!' && i + 2 < chars.len() && chars[i + 1] == '[' && chars[i + 2] == '[' {
            if let Some(end) = find_seq(&chars, i + 3, "]]") {
                flush(&mut buf, &mut spans, base);
                let inner: String = chars[i + 3..end].iter().collect();
                if is_image_ext(link_target_base(&inner)) {
                    spans.push(Span::styled(
                        format!("🖼 {}", link_target_base(&inner)),
                        Style::default().fg(theme.image).add_modifier(Modifier::ITALIC),
                    ));
                } else {
                    let whole: String = chars[i..end + 2].iter().collect();
                    spans.push(Span::styled(
                        whole,
                        Style::default().fg(theme.link).add_modifier(Modifier::UNDERLINED),
                    ));
                }
                i = end + 2;
                continue;
            }
        }
        // ![alt](url) — image
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some((label, next)) = parse_md_link(&chars, i + 1) {
                flush(&mut buf, &mut spans, base);
                spans.push(Span::styled(
                    format!("🖼 {}", label),
                    Style::default().fg(theme.image).add_modifier(Modifier::ITALIC),
                ));
                i = next;
                continue;
            }
        }
        // [[wikilink]]
        if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some(end) = find_seq(&chars, i + 2, "]]") {
                flush(&mut buf, &mut spans, base);
                let inner: String = chars[i..end + 2].iter().collect();
                spans.push(Span::styled(
                    inner,
                    Style::default().fg(theme.link).add_modifier(Modifier::UNDERLINED),
                ));
                i = end + 2;
                continue;
            }
        }
        // [text](url) — show text, drop the url
        if chars[i] == '[' {
            if let Some((label, next)) = parse_md_link(&chars, i) {
                flush(&mut buf, &mut spans, base);
                spans.push(Span::styled(
                    label,
                    Style::default().fg(theme.link).add_modifier(Modifier::UNDERLINED),
                ));
                i = next;
                continue;
            }
        }
        // `code` — strip backticks, dim background
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '`') {
                flush(&mut buf, &mut spans, base);
                let inner: String = chars[i + 1..i + 1 + end].iter().collect();
                spans.push(Span::styled(
                    format!(" {} ", inner),
                    Style::default().fg(theme.code_fg).bg(theme.code_bg),
                ));
                i = i + 1 + end + 1;
                continue;
            }
        }
        // **bold**
        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(end) = find_seq(&chars, i + 2, "**") {
                flush(&mut buf, &mut spans, base);
                let inner: String = chars[i + 2..end].iter().collect();
                spans.push(Span::styled(inner, base.add_modifier(Modifier::BOLD)));
                i = end + 2;
                continue;
            }
        }
        // __bold__ (word-boundary guarded)
        if chars[i] == '_'
            && i + 1 < chars.len()
            && chars[i + 1] == '_'
            && word_boundary_before(&chars, i)
        {
            if let Some(end) = find_seq(&chars, i + 2, "__") {
                flush(&mut buf, &mut spans, base);
                let inner: String = chars[i + 2..end].iter().collect();
                spans.push(Span::styled(inner, base.add_modifier(Modifier::BOLD)));
                i = end + 2;
                continue;
            }
        }
        // ~~strikethrough~~
        if chars[i] == '~' && i + 1 < chars.len() && chars[i + 1] == '~' {
            if let Some(end) = find_seq(&chars, i + 2, "~~") {
                flush(&mut buf, &mut spans, base);
                let inner: String = chars[i + 2..end].iter().collect();
                spans.push(Span::styled(inner, base.add_modifier(Modifier::CROSSED_OUT)));
                i = end + 2;
                continue;
            }
        }
        // *italic* (single)
        if chars[i] == '*' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '*') {
                if end > 0 {
                    flush(&mut buf, &mut spans, base);
                    let inner: String = chars[i + 1..i + 1 + end].iter().collect();
                    spans.push(Span::styled(inner, base.add_modifier(Modifier::ITALIC)));
                    i = i + 1 + end + 1;
                    continue;
                }
            }
        }
        // _italic_ (word-boundary guarded so snake_case survives)
        if chars[i] == '_' && word_boundary_before(&chars, i) {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '_') {
                let after = chars.get(i + 1 + end + 1);
                let boundary_after = after.map(|c| !c.is_alphanumeric()).unwrap_or(true);
                if boundary_after && end > 0 {
                    flush(&mut buf, &mut spans, base);
                    let inner: String = chars[i + 1..i + 1 + end].iter().collect();
                    spans.push(Span::styled(inner, base.add_modifier(Modifier::ITALIC)));
                    i = i + 1 + end + 1;
                    continue;
                }
            }
        }

        buf.push(chars[i]);
        i += 1;
    }
    flush(&mut buf, &mut spans, base);
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    spans
}

fn heading_style(level: u8, theme: &Theme) -> (Style, Option<Color>) {
    match level {
        1 => (Style::default().fg(theme.h1).add_modifier(Modifier::BOLD), Some(theme.h1)),
        2 => (Style::default().fg(theme.h2).add_modifier(Modifier::BOLD), Some(theme.rule)),
        _ => (Style::default().fg(theme.h3).add_modifier(Modifier::BOLD), None),
    }
}

fn word_boundary_before(chars: &[char], i: usize) -> bool {
    i == 0 || !chars[i - 1].is_alphanumeric()
}

/// Parse `[label](url)` starting at the `[`. Returns (label, index after `)`).
fn parse_md_link(chars: &[char], open: usize) -> Option<(String, usize)> {
    if chars.get(open) != Some(&'[') {
        return None;
    }
    let close_rel = chars[open + 1..].iter().position(|&c| c == ']')?;
    let close = open + 1 + close_rel;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let paren_rel = chars[close + 2..].iter().position(|&c| c == ')')?;
    let paren = close + 2 + paren_rel;
    let label: String = chars[open + 1..close].iter().collect();
    let url: String = chars[close + 2..paren].iter().collect();
    let label = if label.trim().is_empty() { url } else { label };
    Some((label, paren + 1))
}

// ---------------------------------------------------------------------------
// Wrapping + display assembly
// ---------------------------------------------------------------------------

/// Word-wrap a styled line into display rows of at most `width` columns,
/// preserving span styles. Continuation rows get `hang` leading spaces.
pub fn wrap_styled_line(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    wrap_styled_line_hang(line, width, 0)
}

pub fn wrap_styled_line_hang(line: &Line<'_>, width: usize, hang: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let hang = hang.min(width / 2);
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
    let mut last_space: Option<usize> = None;

    let flush = |row: &mut Vec<(char, Style)>, rows: &mut Vec<Line<'static>>| {
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

    let indent = |row: &mut Vec<(char, Style)>, row_width: &mut usize| {
        for _ in 0..hang {
            row.push((' ', Style::default()));
        }
        *row_width = hang;
    };

    for (c, style) in chars {
        let cw = c.width().unwrap_or(0);
        if row_width + cw > width {
            match last_space {
                Some(sp) if sp + 1 < row.len() && sp >= hang.min(row.len()) => {
                    let carry: Vec<(char, Style)> = row.split_off(sp + 1);
                    flush(&mut row, &mut rows);
                    indent(&mut row, &mut row_width);
                    row_width += carry.iter().map(|(c, _)| c.width().unwrap_or(0)).sum::<usize>();
                    row.extend(carry);
                }
                _ => {
                    flush(&mut row, &mut rows);
                    indent(&mut row, &mut row_width);
                }
            }
            last_space = None;
            if c == ' ' && row.len() <= hang {
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

/// Final display assembly: search-match highlighting, wrapping with hanging
/// indent, cursor-row mapping.
pub struct DisplayResult {
    pub rows: Vec<Line<'static>>,
    /// Logical content line index per display row (for mouse mapping).
    pub row_logical: Vec<usize>,
    pub cursor_row_start: usize,
    pub cursor_row_end: usize,
}

pub fn build_display(
    doc: &StyledDoc,
    width: usize,
    cursor: usize,
    highlight_cursor: bool,
    query: Option<&str>,
    theme: &Theme,
) -> DisplayResult {
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut row_logical: Vec<usize> = Vec::new();
    let mut cursor_row_start = 0usize;
    let mut cursor_row_end = 0usize;
    let mut cursor_seen = false;

    for (idx, line) in doc.lines.iter().enumerate() {
        let logical = doc.logical[idx];
        let mut line = line.clone();
        if let Some(q) = query {
            if !q.is_empty() {
                line = highlight_matches(&line, q, theme);
            }
        }
        let mut wrapped = if doc.no_wrap[idx] {
            vec![line]
        } else {
            wrap_styled_line_hang(&line, width, doc.hang[idx])
        };
        if logical == cursor {
            if !cursor_seen {
                cursor_row_start = rows.len();
                cursor_seen = true;
            }
            cursor_row_end = rows.len() + wrapped.len().saturating_sub(1);
            if highlight_cursor {
                for row in &mut wrapped {
                    *row = row.clone().style(Style::default().bg(theme.cursor_bg));
                }
            }
        }
        for _ in 0..wrapped.len() {
            row_logical.push(logical);
        }
        rows.extend(wrapped);
    }

    DisplayResult { rows, row_logical, cursor_row_start, cursor_row_end }
}

/// Give every case-insensitive occurrence of `query` a highlight background.
fn highlight_matches(line: &Line<'_>, query: &str, theme: &Theme) -> Line<'static> {
    let chars: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|s| s.content.chars().map(move |c| (c, s.style)))
        .collect();
    let hay: String = chars.iter().map(|(c, _)| c.to_lowercase().next().unwrap_or(*c)).collect();
    let hay_chars: Vec<char> = hay.chars().collect();
    let needle: Vec<char> = query.to_lowercase().chars().collect();
    if needle.is_empty() || hay_chars.len() < needle.len() {
        return line_from_pairs(&chars);
    }

    let mut styled: Vec<(char, Style)> = chars.clone();
    let mut i = 0;
    while i + needle.len() <= hay_chars.len() {
        if hay_chars[i..i + needle.len()] == needle[..] {
            for pair in styled.iter_mut().skip(i).take(needle.len()) {
                pair.1 = Style::default().fg(theme.match_fg).bg(theme.match_bg);
            }
            i += needle.len();
        } else {
            i += 1;
        }
    }
    line_from_pairs(&styled)
}

fn line_from_pairs(pairs: &[(char, Style)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut cur: Option<Style> = None;
    for &(c, style) in pairs {
        if Some(style) != cur {
            if let Some(st) = cur {
                if !buf.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut buf), st));
                }
            }
            cur = Some(style);
        }
        buf.push(c);
    }
    if let Some(st) = cur {
        if !buf.is_empty() {
            spans.push(Span::styled(buf, st));
        }
    }
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Compatibility helpers
// ---------------------------------------------------------------------------

/// Render a Markdown document as styled Text (fixed 80-col layout elements).
/// Kept for tests and simple callers; the viewer uses `style_document`.
pub fn markdown_to_text(md: &str) -> Text<'static> {
    let doc = style_document(md, false, 80, &Theme::dark());
    Text::from(doc.lines)
}

fn find_seq(chars: &[char], from: usize, seq: &str) -> Option<usize> {
    let needle: Vec<char> = seq.chars().collect();
    if needle.is_empty() || chars.len() < needle.len() {
        return None;
    }
    (from..=chars.len() - needle.len()).find(|&i| chars[i..i + needle.len()] == needle[..])
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

    fn row_text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn heading_stripped_and_underlined() {
        let text = markdown_to_text("# Title\nplain");
        // H1 emits heading + underline rows.
        assert_eq!(row_text(&text.lines[0]), "Title");
        assert!(text.lines[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(row_text(&text.lines[1]).starts_with('─'));
        assert_eq!(row_text(&text.lines[2]), "plain");
    }

    #[test]
    fn frontmatter_rendered_as_properties_box() {
        let text = markdown_to_text("---\ntitle: My Note\n---\nbody");
        assert!(row_text(&text.lines[0]).contains("Properties"));
        assert!(row_text(&text.lines[1]).contains("title:"));
        assert!(row_text(&text.lines[2]).starts_with('╰'));
        assert_eq!(row_text(&text.lines[3]), "body");
    }

    #[test]
    fn horizontal_rule_expands() {
        let text = markdown_to_text("a\n\n---\n\nb");
        let rule = text.lines.iter().find(|l| row_text(l).starts_with("───")).unwrap();
        assert!(row_text(rule).chars().all(|c| c == '─'));
    }

    #[test]
    fn wikilink_span_underlined() {
        let text = markdown_to_text("see [[Note A]] now");
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content.as_ref() == "[[Note A]]"
            && s.style.add_modifier.contains(Modifier::UNDERLINED)));
    }

    #[test]
    fn md_link_shows_text_hides_url() {
        let text = markdown_to_text("read [the docs](https://example.com/x) now");
        let all = row_text(&text.lines[0]);
        assert!(all.contains("the docs"));
        assert!(!all.contains("example.com"));
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content.as_ref() == "the docs"
            && s.style.add_modifier.contains(Modifier::UNDERLINED)));
    }

    #[test]
    fn list_bullets_and_numbers() {
        let text = markdown_to_text("- item one\n2. second");
        assert!(row_text(&text.lines[0]).starts_with("• item one"));
        let n = row_text(&text.lines[1]);
        assert!(n.starts_with("2. second"), "{}", n);
        assert!(text.lines[1].spans.iter().any(|s| s.content.as_ref() == "2. "
            && s.style.add_modifier.contains(Modifier::BOLD)));
    }

    #[test]
    fn inline_code_strips_backticks() {
        let text = markdown_to_text("run `cargo test` now");
        let all = row_text(&text.lines[0]);
        assert!(!all.contains('`'));
        assert!(all.contains(" cargo test "));
    }

    #[test]
    fn strikethrough_and_underscore_variants() {
        let text = markdown_to_text("~~gone~~ and __strong__ and _soft_ but snake_case_name stays");
        let spans = &text.lines[0].spans;
        assert!(spans.iter().any(|s| s.content.as_ref() == "gone"
            && s.style.add_modifier.contains(Modifier::CROSSED_OUT)));
        assert!(spans.iter().any(|s| s.content.as_ref() == "strong"
            && s.style.add_modifier.contains(Modifier::BOLD)));
        assert!(spans.iter().any(|s| s.content.as_ref() == "soft"
            && s.style.add_modifier.contains(Modifier::ITALIC)));
        assert!(row_text(&text.lines[0]).contains("snake_case_name"));
    }

    #[test]
    fn fence_labeled_rules_and_gutter() {
        let text = markdown_to_text("```rust\nfn x() {}\n```");
        assert!(row_text(&text.lines[0]).contains("rust"));
        assert!(row_text(&text.lines[0]).starts_with('╭'));
        assert!(row_text(&text.lines[1]).starts_with("│ fn x()"));
        assert!(row_text(&text.lines[2]).starts_with('╰'));
        // Heading inside fence must not be styled as heading.
        let t2 = markdown_to_text("```\n# not a heading\n```");
        assert!(row_text(&t2.lines[1]).contains("# not a heading"));
    }

    #[test]
    fn table_alignment() {
        let md = "| Name | Value |\n|---|---|\n| a | bb |\n| ccc | d |";
        let text = markdown_to_text(md);
        let header = row_text(&text.lines[0]);
        let sep = row_text(&text.lines[1]);
        let r1 = row_text(&text.lines[2]);
        let r2 = row_text(&text.lines[3]);
        assert!(header.starts_with('│') && header.contains("Name"));
        assert!(sep.starts_with('├') && sep.contains('┼'));
        // All rows equal display width (aligned columns).
        assert_eq!(header.chars().count(), r1.chars().count());
        assert_eq!(r1.chars().count(), r2.chars().count());
    }

    #[test]
    fn callout_and_blockquote() {
        let text = markdown_to_text("> [!warning] Careful\n> body line\n\n> plain quote");
        assert!(row_text(&text.lines[0]).contains("Warning: Careful"));
        assert!(row_text(&text.lines[1]).starts_with('┃'));
        let quote = text.lines.iter().find(|l| row_text(l).contains("plain quote")).unwrap();
        assert!(row_text(quote).starts_with('▌'));
    }

    #[test]
    fn checkbox_colors_and_marks() {
        let text = markdown_to_text("- [x] done thing");
        let all = row_text(&text.lines[0]);
        assert!(all.starts_with("☑"), "{}", all);
        assert!(text.lines[0].spans.iter().any(|s| s.style.fg == Some(Color::Green)));
    }

    #[test]
    fn image_placeholders() {
        let text = markdown_to_text("a ![[photo.png]] b ![a cat](cat.jpg) c ![[Some Note]]");
        let all = row_text(&text.lines[0]);
        assert!(all.contains("🖼 photo.png"));
        assert!(all.contains("🖼 a cat"));
        assert!(all.contains("![[Some Note]]"));
    }

    #[test]
    fn links_in_line() {
        assert_eq!(wikilinks_in_line("a [[X]] b [[Y|y]]"), vec!["X", "Y"]);
    }

    // ---- wrapping ----

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
        assert!(joined
            .split_whitespace()
            .eq("the quick brown fox jumps over the lazy dog".split_whitespace()));
    }

    #[test]
    fn wrap_preserves_span_styles_across_break() {
        let line = Line::from(vec![
            Span::raw("plain start "),
            Span::styled(
                "bold segment that will wrap",
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]);
        let rows = wrap_styled_line(&line, 18);
        assert!(rows.len() >= 2);
        let last = rows.last().unwrap();
        assert!(last.spans.iter().all(|s| s.style.add_modifier.contains(Modifier::BOLD)));
    }

    #[test]
    fn wrap_hard_breaks_single_long_word_and_handles_empty() {
        let rows = wrap_styled_line(&Line::from("abcdefghij"), 4);
        assert_eq!(rows.iter().map(row_text).collect::<Vec<_>>(), vec!["abcd", "efgh", "ij"]);
        let rows = wrap_styled_line(&Line::from(""), 10);
        assert_eq!(rows.len(), 1);
        let rows = wrap_styled_line(&Line::from("short"), 40);
        assert_eq!(rows.len(), 1);
        assert_eq!(row_text(&rows[0]), "short");
    }

    #[test]
    fn wrap_hanging_indent_aligns_continuations() {
        let line = Line::from("• one two three four five six seven eight");
        let rows = wrap_styled_line_hang(&line, 14, 2);
        assert!(rows.len() > 1);
        for r in &rows[1..] {
            let t = row_text(r);
            assert!(t.starts_with("  "), "continuation not indented: {:?}", t);
            assert!(t.chars().count() <= 14);
        }
    }

    // ---- build_display ----

    #[test]
    fn build_display_maps_cursor_and_logical_rows() {
        let md = "# Head\nshort\nthis is a much longer line that will certainly wrap at narrow width";
        let doc = style_document(md, false, 20, &Theme::dark());
        let d = build_display(&doc, 20, 2, true, None, &Theme::dark());
        // Cursor (logical line 2) spans multiple display rows, all mapped to 2.
        assert!(d.cursor_row_end > d.cursor_row_start);
        for r in d.cursor_row_start..=d.cursor_row_end {
            assert_eq!(d.row_logical[r], 2);
        }
        // Rows and map stay in sync.
        assert_eq!(d.rows.len(), d.row_logical.len());
    }

    #[test]
    fn build_display_highlights_search_matches() {
        let doc = style_document("alpha beta alpha", false, 40, &Theme::dark());
        let theme = Theme::dark();
        let d = build_display(&doc, 40, 0, false, Some("alpha"), &theme);
        let matched: Vec<_> = d.rows[0]
            .spans
            .iter()
            .filter(|s| s.style.bg == Some(theme.match_bg))
            .collect();
        assert_eq!(matched.len(), 2, "both occurrences highlighted");
        assert!(matched.iter().all(|s| s.content.as_ref() == "alpha"));
    }

    #[test]
    fn raw_document_is_literal() {
        let doc = style_document("# Title\n**bold**", true, 40, &Theme::dark());
        assert_eq!(row_text(&Line::from(doc.lines[0].clone())), "# Title");
        assert_eq!(row_text(&Line::from(doc.lines[1].clone())), "**bold**");
    }
}

#[cfg(test)]
mod perf_probe {
    use super::*;

    #[test]
    fn style_document_is_fast_enough() {
        let md = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("developer_examples/output.md"),
        )
        .unwrap_or_else(|_| "# t\ntext\n".repeat(500));
        let theme = Theme::dark();
        let start = std::time::Instant::now();
        for _ in 0..10 {
            let doc = style_document(&md, false, 78, &theme);
            let _ = build_display(&doc, 78, 5, true, None, &theme);
        }
        let per_frame = start.elapsed() / 10;
        println!("per-frame style+display: {:?}", per_frame);
        assert!(per_frame < std::time::Duration::from_millis(50), "too slow: {:?}", per_frame);
    }
}

#[cfg(test)]
mod syntect_probe {
    use super::*;

    #[test]
    fn syntect_load_and_highlight_time() {
        let start = std::time::Instant::now();
        once_cell::sync::Lazy::force(&SYNTECT);
        println!("syntect load: {:?}", start.elapsed());
        let theme = Theme::dark();
        let start = std::time::Instant::now();
        let doc = style_document("```rust\nfn main() { let x: u32 = 42; }\n```", false, 80, &theme);
        println!("first fence doc: {:?}; lines {}", start.elapsed(), doc.lines.len());
    }
}
