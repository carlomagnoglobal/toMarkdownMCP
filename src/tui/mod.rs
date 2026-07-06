mod app;
pub mod render;

use anyhow::{anyhow, Result};
use app::{App, Focus, SearchTarget};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use std::path::{Path, PathBuf};

/// Restores the terminal even on panic/early return.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

/// Run the TUI viewer over a vault directory or a single Markdown file.
pub fn run(path: &Path) -> Result<()> {
    let (root, initial): (PathBuf, Option<String>) = if path.is_dir() {
        (path.to_path_buf(), None)
    } else if path.is_file() {
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let rel = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| anyhow!("Invalid file path"))?;
        (root, Some(rel))
    } else {
        return Err(anyhow!("Path not found: {}", path.display()));
    };

    let (files, _) = crate::obsidian::vault::vault_walk(&root)?;
    if files.is_empty() {
        return Err(anyhow!("No Markdown files found under {}", root.display()));
    }
    let mut app = App::new(root, files);
    if let Some(rel) = initial {
        app.open(&rel, false);
    }

    enable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?;

    while !app.quit {
        app.maybe_reload();
        terminal.draw(|f| draw(f, &mut app))?;
        if event::poll(std::time::Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key(&mut app, key.code, key.modifiers);
                }
                Event::Mouse(m) => handle_mouse(&mut app, m),
                _ => {}
            }
        }
    }
    Ok(())
}

fn handle_mouse(app: &mut App, m: crossterm::event::MouseEvent) {
    let in_tree = !app.zen
        && m.column >= app.tree_area.x
        && m.column < app.tree_area.x + app.tree_area.width
        && m.row >= app.tree_area.y
        && m.row < app.tree_area.y + app.tree_area.height;
    let in_content = m.column >= app.content_area.x
        && m.column < app.content_area.x + app.content_area.width
        && m.row >= app.content_area.y
        && m.row < app.content_area.y + app.content_area.height;

    match m.kind {
        MouseEventKind::ScrollDown => {
            if in_tree {
                app.selected = (app.selected + 3).min(app.filtered.len().saturating_sub(1));
                app.focus = Focus::Tree;
            } else if in_content {
                let max = app.content.lines().count().saturating_sub(1);
                app.cursor = (app.cursor + 3).min(max);
                app.focus = Focus::Content;
            }
        }
        MouseEventKind::ScrollUp => {
            if in_tree {
                app.selected = app.selected.saturating_sub(3);
                app.focus = Focus::Tree;
            } else if in_content {
                app.cursor = app.cursor.saturating_sub(3);
                app.focus = Focus::Content;
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if in_tree {
                app.focus = Focus::Tree;
                // Click row -> rendered tree row (account for the list's scroll offset).
                let rel = (m.row - app.tree_area.y).saturating_sub(1) as usize;
                let row = app.tree_state.offset() + rel;
                if let Some(Some(file_pos)) = app.tree_rows.get(row) {
                    app.selected = *file_pos;
                    if let Some(relpath) = app.selected_file().cloned() {
                        app.open(&relpath, true);
                    }
                }
            } else if in_content {
                app.focus = Focus::Content;
                let rel = (m.row - app.content_area.y).saturating_sub(1) as usize;
                let display_row = app.scroll as usize + rel;
                if let Some(&logical) = app.row_logical.get(display_row) {
                    if logical == app.cursor {
                        // Second click on the same line follows its wikilink.
                        app.follow_link();
                    } else {
                        app.cursor = logical;
                    }
                }
            }
        }
        _ => {}
    }
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    // Any key dismisses the help overlay, except '?' which toggles it below.
    if app.show_help && code != KeyCode::Char('?') {
        app.show_help = false;
        return;
    }
    // Same for the stats popup ('s' re-toggles).
    if app.show_stats.is_some() && code != KeyCode::Char('s') {
        app.show_stats = None;
        return;
    }

    if app.searching {
        match code {
            KeyCode::Esc => app.cancel_search(),
            KeyCode::Enter => app.commit_search(),
            KeyCode::Backspace => app.backspace_search(),
            KeyCode::Char(c) => app.type_search_char(c),
            _ => {}
        }
        return;
    }

    match (code, mods) {
        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.quit = true,
        (KeyCode::Char('?'), _) => app.toggle_help(),
        (KeyCode::Char('r'), _) => app.toggle_raw_view(),
        (KeyCode::Char('z'), _) => app.toggle_zen(),
        (KeyCode::Char('T'), _) => app.cycle_theme(),
        (KeyCode::Char('s'), KeyModifiers::NONE) => app.toggle_stats(),
        (KeyCode::Tab, _) | (KeyCode::BackTab, _) => {
            app.focus = if app.focus == Focus::Tree { Focus::Content } else { Focus::Tree };
        }
        (KeyCode::Char('/'), _) => app.begin_search(),
        (KeyCode::Backspace, _) | (KeyCode::Esc, _) => app.back(),
        _ => match app.focus {
            Focus::Tree => {
                let page = 10usize;
                let last = app.filtered.len().saturating_sub(1);
                match (code, mods) {
                    (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                        app.selected = app.selected.saturating_sub(1);
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                        if app.selected + 1 < app.filtered.len() {
                            app.selected += 1;
                        }
                    }
                    (KeyCode::Char('h'), KeyModifiers::NONE) => app.back(),
                    (KeyCode::Char('g'), KeyModifiers::NONE) | (KeyCode::Home, _) => app.selected = 0,
                    (KeyCode::Char('G'), _) | (KeyCode::End, _) => app.selected = last,
                    (KeyCode::PageDown, _)
                    | (KeyCode::Char('f'), KeyModifiers::CONTROL)
                    | (KeyCode::Char(' '), KeyModifiers::NONE) => {
                        app.selected = (app.selected + page).min(last);
                    }
                    (KeyCode::PageUp, _) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                        app.selected = app.selected.saturating_sub(page);
                    }
                    (KeyCode::Enter, _) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
                        if let Some(rel) = app.selected_file().cloned() {
                            app.open(&rel, true);
                        }
                    }
                    _ => {}
                }
            }
            Focus::Content => {
                let page = app.view_height.max(1);
                let half = (page / 2).max(1);
                let max_line = app.content.lines().count().saturating_sub(1);
                match (code, mods) {
                    (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                        app.cursor = app.cursor.saturating_sub(1);
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                        if app.cursor < max_line {
                            app.cursor += 1;
                        }
                    }
                    (KeyCode::Char('h'), KeyModifiers::NONE) => app.back(),
                    (KeyCode::Char('l'), KeyModifiers::NONE) => app.follow_link(),
                    (KeyCode::PageDown, _)
                    | (KeyCode::Char('f'), KeyModifiers::CONTROL)
                    | (KeyCode::Char(' '), KeyModifiers::NONE) => {
                        app.cursor = (app.cursor + page).min(max_line);
                    }
                    (KeyCode::PageUp, _) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                        app.cursor = app.cursor.saturating_sub(page);
                    }
                    (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                        app.cursor = (app.cursor + half).min(max_line);
                    }
                    (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                        app.cursor = app.cursor.saturating_sub(half);
                    }
                    (KeyCode::Char('g'), KeyModifiers::NONE) | (KeyCode::Home, _) => app.cursor = 0,
                    (KeyCode::Char('G'), _) | (KeyCode::End, _) => app.cursor = max_line,
                    (KeyCode::Char('n'), KeyModifiers::NONE) => app.next_match(),
                    (KeyCode::Char('N'), _) => app.prev_match(),
                    (KeyCode::Enter, _) => app.follow_link(),
                    _ => {}
                }
            }
        },
    }
}

/// Widest readable text column; wider panes center the text block.
const MAX_TEXT_WIDTH: u16 = 100;

fn draw(f: &mut ratatui::Frame, app: &mut App) {
    let theme = render::Theme::by_index(app.theme_idx);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(f.area());

    let content_pane = if app.zen {
        app.tree_area = Rect::default();
        outer[0]
    } else {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(outer[0]);
        app.tree_area = panes[0];
        draw_tree(f, app, panes[0], &theme);
        panes[1]
    };
    app.content_area = content_pane;

    let (total_rows, _nearest_heading) = draw_content(f, app, content_pane, &theme);
    draw_status(f, app, outer[1], total_rows, &theme);

    if let Some(stats) = &app.show_stats {
        draw_stats(f, f.area(), stats, &theme);
    }
    if app.show_help {
        draw_help(f, f.area());
    }
}

fn draw_stats(f: &mut ratatui::Frame, area: Rect, stats: &app::NoteStats, theme: &render::Theme) {
    let m = &stats.openai;
    let mut lines: Vec<Line> = Vec::new();
    let key = Style::default().fg(theme.h2).add_modifier(Modifier::BOLD);

    lines.push(Line::from(vec![
        Span::styled("  Words ", key),
        Span::raw(m.words.to_string()),
        Span::styled("   Chars ", key),
        Span::raw(m.chars.to_string()),
        Span::styled("   Spaces ", key),
        Span::raw(m.spaces.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Tokens ", key),
        Span::raw(format!("{} (gpt-4o o200k, exact)", m.tokens)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("         ", key),
        Span::raw(format!("~{} (Claude cl100k proxy, ", stats.anthropic_tokens)),
        Span::styled("estimate", Style::default().fg(theme.quote)),
        Span::raw(")"),
    ]));
    lines.push(Line::from(""));

    // Three columns: top words | top chars | top tokens
    lines.push(Line::from(vec![
        Span::styled(format!("  {:<26}", "── top words ──"), Style::default().fg(theme.rule)),
        Span::styled(format!("{:<18}", "── top chars ──"), Style::default().fg(theme.rule)),
        Span::styled("── top tokens ──", Style::default().fg(theme.rule)),
    ]));
    let rows = 12usize;
    for i in 0..rows {
        let w = m
            .word_freq
            .get(i)
            .map(|(w, c)| format!("{} = {}", truncate_middle(w, 17), c))
            .unwrap_or_default();
        let ch = m
            .char_freq
            .get(i)
            .map(|(ch, c)| format!("{} = {}", ch, c))
            .unwrap_or_default();
        let t = m
            .token_freq
            .get(i)
            .map(|(t, c)| format!("{} = {}", truncate_middle(t, 17), c))
            .unwrap_or_default();
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<26}", w)),
            Span::raw(format!("{:<18}", ch)),
            Span::raw(t),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  s or any key closes · analyze_text MCP tool has full tables",
        Style::default().fg(theme.text_dim),
    )));

    let rect = centered_rect(74, (lines.len() + 2) as u16, area);
    f.render_widget(Clear, rect);
    let popup = Paragraph::new(ratatui::text::Text::from(lines)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(" Text Stats ")
            .border_style(Style::default().fg(theme.border_focused)),
    );
    f.render_widget(popup, rect);
}

fn border_style(focused: bool, theme: &render::Theme) -> Style {
    if focused {
        Style::default().fg(theme.border_focused)
    } else {
        Style::default().fg(theme.border_unfocused)
    }
}

fn draw_tree(f: &mut ratatui::Frame, app: &mut App, area: Rect, theme: &render::Theme) {
    // Group flat vault-relative paths by directory. Also record the rendered
    // row -> file mapping for mouse clicks.
    let mut items: Vec<ListItem> = Vec::new();
    let mut tree_rows: Vec<Option<usize>> = Vec::new();
    let mut selected_row = 0usize;
    let mut last_dir: Option<&str> = None;
    for (fi, &idx) in app.filtered.iter().enumerate() {
        let path = &app.files[idx];
        let (dir, name) = path.rsplit_once('/').unwrap_or(("", path.as_str()));
        if last_dir != Some(dir) {
            if !dir.is_empty() {
                items.push(ListItem::new(Line::from(Span::styled(
                    format!("▸ {}/", dir),
                    Style::default().fg(theme.tree_dir).add_modifier(Modifier::BOLD),
                ))));
                tree_rows.push(None);
            }
            last_dir = Some(dir);
        }
        if fi == app.selected {
            selected_row = items.len();
        }
        let indent = if dir.is_empty() { "" } else { "  " };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{}· ", indent), Style::default().fg(theme.text_dim)),
            Span::raw(name.to_string()),
        ])));
        tree_rows.push(Some(fi));
    }
    app.tree_rows = tree_rows;

    let tree_title = if app.searching && app.search_target == SearchTarget::Tree {
        format!(" Files /{}▏", app.search)
    } else {
        format!(" Files ({}) ", app.filtered.len())
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(tree_title)
                .border_style(border_style(app.focus == Focus::Tree, theme)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));
    if app.filtered.is_empty() {
        app.tree_state.select(None);
    } else {
        app.tree_state.select(Some(selected_row));
    }
    let mut state = std::mem::take(&mut app.tree_state);
    f.render_stateful_widget(list, area, &mut state);
    app.tree_state = state;
}

/// Renders the content pane. Returns (total display rows, nearest heading).
fn draw_content(
    f: &mut ratatui::Frame,
    app: &mut App,
    area: Rect,
    theme: &render::Theme,
) -> (usize, Option<String>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(border_style(app.focus == Focus::Content, theme));

    // Reading-width cap: center the text block when the pane is wide.
    let inner = block.inner(area);
    let text_width = inner.width.min(MAX_TEXT_WIDTH);
    let text_area = Rect {
        x: inner.x + (inner.width - text_width) / 2,
        y: inner.y,
        width: text_width,
        height: inner.height,
    };
    let inner_width = text_width.max(1) as usize;

    let doc = app.styled_doc(inner_width, theme);
    let query = if !app.content_matches.is_empty()
        || (app.searching && app.search_target == SearchTarget::Content)
    {
        Some(app.search.as_str())
    } else {
        None
    };
    let d = render::build_display(
        &doc,
        inner_width,
        app.cursor,
        app.focus == Focus::Content,
        query,
        theme,
    );
    let total_rows = d.rows.len();
    app.row_logical = d.row_logical.clone();

    // Keep the cursor's display rows visible.
    let view_height = inner.height;
    app.view_height = view_height as usize;
    if (d.cursor_row_end as u16) >= app.scroll + view_height {
        app.scroll = d.cursor_row_end as u16 - view_height + 1;
    }
    if (d.cursor_row_start as u16) < app.scroll {
        app.scroll = d.cursor_row_start as u16;
    }

    // Title: note · breadcrumb/tags · state, trimmed to fit.
    let nearest_heading = nearest_heading_above(&app.content, app.cursor);
    let title = content_title(app, nearest_heading.as_deref());
    let title = truncate_middle(&title, area.width.saturating_sub(4) as usize);

    f.render_widget(block.clone().title(title), area);
    let para = Paragraph::new(ratatui::text::Text::from(d.rows)).scroll((app.scroll, 0));
    f.render_widget(para, text_area);

    // Scrollbar on the pane's right edge.
    if total_rows > view_height as usize {
        let mut sb_state = ratatui::widgets::ScrollbarState::new(
            total_rows.saturating_sub(view_height as usize),
        )
        .position(app.scroll as usize);
        f.render_stateful_widget(
            ratatui::widgets::Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_style(Style::default().fg(theme.border_focused))
                .track_style(Style::default().fg(theme.border_unfocused)),
            area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
            &mut sb_state,
        );
    }

    (total_rows, nearest_heading)
}

fn content_title(app: &App, heading: Option<&str>) -> String {
    match &app.current {
        Some(rel) => {
            let mut title = format!(" {}", rel);
            if let Some(h) = heading {
                if app.cursor > 0 {
                    title.push_str(&format!(" › {}", h));
                }
            }
            if !app.current_tags.is_empty() {
                title.push_str(&format!(" · #{}", app.current_tags.join(" #")));
            }
            if app.raw_view {
                title.push_str(" · raw");
            }
            if app.searching && app.search_target == SearchTarget::Content {
                title.push_str(&format!(" · /{}▏", app.search));
            }
            title.push(' ');
            title
        }
        None => " (no note) ".to_string(),
    }
}

/// The nearest Markdown heading at or above the cursor line (outside fences).
fn nearest_heading_above(content: &str, cursor: usize) -> Option<String> {
    let mut in_fence = false;
    let mut found: Option<String> = None;
    for (i, line) in content.lines().enumerate() {
        if i > cursor {
            break;
        }
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let hashes = t.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && t.chars().nth(hashes) == Some(' ') {
            found = Some(t[hashes + 1..].trim().to_string());
        }
    }
    found
}

fn truncate_middle(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max || max < 5 {
        return s.to_string();
    }
    let keep = max - 1;
    let head: String = s.chars().take(keep).collect();
    format!("{}…", head)
}

fn draw_status(
    f: &mut ratatui::Frame,
    app: &App,
    area: Rect,
    total_rows: usize,
    theme: &render::Theme,
) {
    let lines_total = app.content.lines().count().max(1);
    let pct = if lines_total <= 1 { 100 } else { (app.cursor * 100) / (lines_total - 1) };
    let minutes = (app.word_count + 219) / 220;
    let _ = total_rows;
    let right = if app.current.is_some() {
        format!(
            "L{}/{} · {}% · {}w · ~{}min ",
            app.cursor + 1,
            lines_total,
            pct,
            app.word_count,
            minutes.max(1),
        )
    } else {
        String::new()
    };

    let left_width = area.width.saturating_sub(right.chars().count() as u16);
    let left = truncate_middle(&app.status, left_width.saturating_sub(1) as usize);
    let pad = area
        .width
        .saturating_sub(left.chars().count() as u16 + right.chars().count() as u16);
    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(theme.status_fg)),
        Span::raw(" ".repeat(pad as usize)),
        Span::styled(right, Style::default().fg(theme.status_fg)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

fn draw_help(f: &mut ratatui::Frame, area: Rect) {
    const HELP: &str = "\
Navigation
  j/k, ↑/↓        move line/file
  h / l           back / open · follow link
  Space, Ctrl+f   page down       Ctrl+b   page up
  Ctrl+d          half-page down  Ctrl+u   half-page up
  g / G           top / bottom    Home/End same
  Tab, Shift+Tab  switch pane

Search
  /               search (tree: filter files · content: find in note)
  n / N           next / previous match
  Enter           confirm search   Esc   cancel search

Notes
  Enter           open file / follow [[wikilink]] under cursor
  Backspace, Esc  go back
  r               toggle raw source / formatted view

View
  z               zen mode (hide file tree)
  T               cycle theme (dark / light)
  s               text stats popup (words/chars/spaces/tokens + top
                  word & token frequencies for the open note)
  Mouse           wheel scrolls · click selects/opens · click twice
                  on a line to follow its [[wikilink]]

General
  ?               toggle this help
  q, Ctrl+c       quit
";
    let rect = centered_rect(64, 30, area);
    f.render_widget(Clear, rect);
    let popup = Paragraph::new(HELP)
        .block(Block::default().borders(Borders::ALL).title(" Help (any key closes) "))
        .style(Style::default().fg(Color::White));
    f.render_widget(popup, rect);
}
