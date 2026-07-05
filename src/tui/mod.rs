mod app;
pub mod render;

use anyhow::{anyhow, Result};
use app::{App, Focus};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
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
    if app.searching {
        match code {
            KeyCode::Esc => {
                app.searching = false;
                app.search.clear();
                app.apply_filter();
            }
            KeyCode::Enter => app.searching = false,
            KeyCode::Backspace => {
                app.search.pop();
                app.apply_filter();
            }
            KeyCode::Char(c) => {
                app.search.push(c);
                app.apply_filter();
            }
            _ => {}
        }
        return;
    }

    match (code, mods) {
        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.quit = true,
        (KeyCode::Tab, _) => {
            app.focus = if app.focus == Focus::Tree { Focus::Content } else { Focus::Tree };
        }
        (KeyCode::Char('/'), _) => {
            app.searching = true;
            app.focus = Focus::Tree;
        }
        (KeyCode::Backspace, _) => app.back(),
        _ => match app.focus {
            Focus::Tree => match code {
                KeyCode::Up | KeyCode::Char('k') => app.selected = app.selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.selected + 1 < app.filtered.len() {
                        app.selected += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(rel) = app.selected_file().cloned() {
                        app.open(&rel, true);
                    }
                }
                _ => {}
            },
            Focus::Content => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    app.cursor = app.cursor.saturating_sub(1);
                    app.scroll = app.scroll.min(app.cursor as u16);
                    if (app.cursor as u16) < app.scroll {
                        app.scroll = app.cursor as u16;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = app.content.lines().count().saturating_sub(1);
                    if app.cursor < max {
                        app.cursor += 1;
                    }
                }
                KeyCode::PageDown => app.cursor = (app.cursor + 20).min(app.content.lines().count().saturating_sub(1)),
                KeyCode::PageUp => app.cursor = app.cursor.saturating_sub(20),
                KeyCode::Enter => app.follow_link(),
                _ => {}
            },
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
    let tree_title = if app.searching || !app.search.is_empty() {
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

    // Content with cursor-line highlight
    let mut text = render::markdown_to_text(&app.content);
    if app.focus == Focus::Content {
        if let Some(line) = text.lines.get_mut(app.cursor) {
            *line = line.clone().style(Style::default().bg(Color::Rgb(40, 40, 60)));
        }
    }
    // Keep cursor visible
    let view_height = panes[1].height.saturating_sub(2);
    if (app.cursor as u16) >= app.scroll + view_height {
        app.scroll = app.cursor as u16 - view_height + 1;
    }
    if (app.cursor as u16) < app.scroll {
        app.scroll = app.cursor as u16;
    }
    let content_title = format!(" {} ", app.current.as_deref().unwrap_or("(no note)"));
    let content_style = if app.focus == Focus::Content {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(content_title).border_style(content_style))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0));
    f.render_widget(para, panes[1]);

    // Status bar
    f.render_widget(
        Paragraph::new(Line::from(app.status.clone())).style(Style::default().fg(Color::DarkGray)),
        outer[1],
    );
}
