# toMarkdown Viewer (desktop GUI)

Desktop viewer for Markdown, vaults, and every format the converters support (PDF, DOCX, HTML, EPUB, …) — a Tauri app in `gui/`, sharing all conversion/vault logic with the MCP server through the `to_markdown_mcp` library crate.

## Status

Phase 5 of the [roadmap](../planning/ROADMAP.md): foundation & viewer. Later phases add live file watching, Obsidian-grade vault navigation (backlinks, graph, search), Typora-style editing, and AI features.

## Current features

- **File tree sidebar** — open a folder (native picker, drag-and-drop, or recents); directories collapse/expand; hidden and build directories are filtered out
- **Rendered Markdown pane** — headings, tables, task lists, footnotes, strikethrough (pulldown-cmark); code and config files render as fenced code blocks with detected language
- **Any supported format** — non-Markdown files (PDF, DOCX, XLSX, EPUB, HTML, …) are converted to Markdown by the shared pipeline before rendering
- **Light/dark theme** — follows the OS automatically
- **Recent files & folders** — persisted locally, one click to reopen

## Build & run

The GUI is excluded from default workspace builds (`cargo build` still builds only the MCP server). Build it explicitly:

```bash
cargo build -p to_markdown_gui            # compile
cargo run -p to_markdown_gui              # launch the viewer
```

Packaging a macOS `.app` / installers uses the Tauri CLI (`cargo install tauri-cli`, then `cargo tauri build` from `gui/`). Bundling is disabled in `gui/tauri.conf.json` (`bundle.active: false`) until the packaging pass.

## Architecture

- `gui/src/main.rs` — Tauri commands: `list_tree` (recursive, sorted, filtered), `open_file` (convert → Markdown → HTML), `pick_folder`/`pick_file` (native dialogs)
- `gui/ui/index.html` — single-file vanilla JS frontend (no npm/bundler); talks to Rust via `window.__TAURI__` (`withGlobalTauri`)
- `src/lib.rs` / `src/pipeline.rs` — the shared library: all converter modules plus `convert_any_to_markdown` with the large-file guardrails
