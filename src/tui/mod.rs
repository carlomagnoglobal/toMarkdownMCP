mod app;
pub mod render;

use anyhow::{anyhow, Result};
use app::{App, Focus, SearchTarget};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use std::path::{Path, PathBuf};

/// Restores the terminal even on panic/early return.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
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
    crossterm::execute!(std::io::stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?;

    while !app.quit {
        app.maybe_reload();
        terminal.draw(|f| draw(f, &mut app))?;
        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(&mut app, key.code, key.modifiers);
                }
            }
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    // Any key dismisses the help overlay, except '?' which toggles it below.
    if app.show_help && code != KeyCode::Char('?') {
        app.show_help = false;
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

fn draw(f: &mut ratatui::Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(f.area());
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(outer[0]);

    // File tree
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| ListItem::new(app.files[i].clone()))
        .collect();
    let tree_title = if app.searching && app.search_target == SearchTarget::Tree {
        format!(" Files /{} ", app.search)
    } else {
        " Files ".to_string()
    };
    let tree_style = if app.focus == Focus::Tree {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(tree_title).border_style(tree_style))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));
    let mut state = ListState::default();
    state.select(if app.filtered.is_empty() { None } else { Some(app.selected) });
    f.render_stateful_widget(list, panes[0], &mut state);

    // Content: wrap lines ourselves so scrolling is exact in display rows.
    // (Paragraph's own Wrap scrolls in post-wrap rows while the cursor moves
    // in logical lines — mixing the two loses the cursor on narrow terminals.)
    let logical: Vec<Line> = if app.raw_view {
        app.content.lines().map(Line::from).collect()
    } else {
        render::markdown_to_text(&app.content).lines
    };
    let inner_width = panes[1].width.saturating_sub(2).max(1) as usize;
    let mut display: Vec<Line<'static>> = Vec::new();
    let mut cursor_row_start = 0usize;
    let mut cursor_row_end = 0usize;
    for (i, line) in logical.iter().enumerate() {
        let mut rows = render::wrap_styled_line(line, inner_width);
        if i == app.cursor {
            cursor_row_start = display.len();
            cursor_row_end = display.len() + rows.len().saturating_sub(1);
            if app.focus == Focus::Content {
                for row in &mut rows {
                    *row = row.clone().style(Style::default().bg(Color::Rgb(40, 40, 60)));
                }
            }
        }
        display.extend(rows);
    }
    let text = ratatui::text::Text::from(display);

    // Keep the cursor's display rows visible.
    let view_height = panes[1].height.saturating_sub(2);
    app.view_height = view_height as usize;
    if (cursor_row_end as u16) >= app.scroll + view_height {
        app.scroll = cursor_row_end as u16 - view_height + 1;
    }
    if (cursor_row_start as u16) < app.scroll {
        app.scroll = cursor_row_start as u16;
    }
    let content_title = match &app.current {
        Some(rel) => {
            let tags = if app.current_tags.is_empty() {
                String::new()
            } else {
                format!(" · #{}", app.current_tags.join(" #"))
            };
            let mode = if app.raw_view { " · raw" } else { "" };
            let search = if app.searching && app.search_target == SearchTarget::Content {
                format!(" · /{}", app.search)
            } else {
                String::new()
            };
            format!(" {}{}{}{} · {} backlinks ", rel, tags, mode, search, app.current_backlink_count)
        }
        None => " (no note) ".to_string(),
    };
    let content_style = if app.focus == Focus::Content {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(content_title).border_style(content_style))
        .scroll((app.scroll, 0));
    f.render_widget(para, panes[1]);

    // Status bar
    f.render_widget(
        Paragraph::new(Line::from(app.status.clone())).style(Style::default().fg(Color::DarkGray)),
        outer[1],
    );

    if app.show_help {
        draw_help(f, f.area());
    }
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

General
  ?               toggle this help
  q, Ctrl+c       quit
";
    let rect = centered_rect(60, 24, area);
    f.render_widget(Clear, rect);
    let popup = Paragraph::new(HELP)
        .block(Block::default().borders(Borders::ALL).title(" Help (any key closes) "))
        .style(Style::default().fg(Color::White));
    f.render_widget(popup, rect);
}
