# GUI Polish & Hybrid Editing — Design

**Date:** 2026-07-17
**Component:** `gui/` (to_markdown_gui, Tauri 2 desktop app)
**Goal:** Make the GUI a world-class markdown viewer/editor, competing on polish and UX quality with Typora and Obsidian.

## Context

The GUI (v0.2.0) already has: file tree, rendered markdown view (pulldown-cmark + syntect), vendored Mermaid + KaTeX, file watching, a raw-source editor, Obsidian-style vault features (wikilinks, backlinks, graph, tags, daily notes, templates), AI actions, text metrics, and HTML/DOCX/RTF export. The gap is not features — it is refinement: typography, keyboard-driven workflow, reading experience, and a modern live-preview editing model.

**Hard constraint:** the frontend stays build-free vanilla JS. No npm, no bundler in the dev or CI workflow. Third-party libraries are vendored as prebuilt minified files (as Mermaid and KaTeX already are).

**Approach chosen:** polish first (Phase 1), hybrid editor second (Phase 2), with incremental modularization of the UI code as each area is touched.

## Architecture

- Tauri 2 app unchanged; rendering stays server-side in `gui/src/render.rs` for reader mode.
- As each feature area is touched, its JS is extracted from `gui/ui/index.html` into plain ES modules under `gui/ui/js/` (e.g. `theme.js`, `palette.js`, `toc.js`, `editor.js`), loaded via `<script type="module">`. No bundler.
- Vendored dependencies live in `gui/ui/vendor/`. Phase 2 adds a one-time prebuilt CodeMirror 6 bundle (built once outside the repo, committed as a minified ESM file with its license).
- Settings (theme, font size, editor mode, etc.) persist as JSON in the Tauri app-config directory via two new commands: `get_settings` / `set_settings`. Other backend additions are small commands appended to the existing `invoke_handler`.

## Phase 1 — Reading & polish

### 1. Typography & themes
- Type system for the rendered view: ~68ch measure, adjustable font size and line height (Cmd +/−, persisted), improved heading rhythm, table and code-block styling.
- Theme system on CSS custom properties: light, dark, sepia, and "follow system". Smooth transition on switch.
- User override: if `custom.css` exists in the app-config dir, it is loaded after the theme.

### 2. Reading experience
- Floating, collapsible outline/TOC built from rendered headings, scroll-synced with the document.
- Focus/zen mode: hides chrome, centers the column.
- Reading-progress indicator.
- Image lightbox (click to zoom).
- Print stylesheet producing clean page output.

### 3. Keyboard-first UX
- Cmd+K command palette exposing every app action (exports, daily note, graph, search, theme switch, mode toggle, …).
- Cmd+P quick file switcher over the vault tree (fuzzy match).
- Complete shortcut coverage; `?` opens a shortcut cheat-sheet overlay.

### 4. Micro-interactions
- Hover affordances on tree items and links; drag-and-drop file open onto the window.
- Polished empty states.
- Non-blocking toast notifications for errors/success, replacing blocking dialogs (also avoids the known sync-dialog deadlock on Tauri).

## Phase 2 — Hybrid editing modes (Rust-first, revised 2026-07-17)

**Language policy (revised):** all editor intelligence is implemented in Rust. No third-party JavaScript editor libraries (no CodeMirror or equivalents), no npm-built bundles beyond the already-vendored Mermaid/KaTeX. The only JavaScript is thin hand-written glue in `gui/ui/index.html` that displays what Rust computes and forwards input events to Rust commands. Other languages (C/C++, Go, Dart, Python, R, Node) are permitted only for optional plugins when a capability does not exist in Rust or is disproportionately hard there.

Per-document mode, cycled with Cmd+E or set from the palette:

1. **Reader** — the current rendered view; pipeline untouched.
2. **Live preview** (Typora-style) — evolves the existing Rust-backed block editor (`render_blocks`): every block renders through the Rust pipeline (including Mermaid/KaTeX/images); the block under the cursor swaps to editable source; Rust computes markdown token spans so even the active block shows syntax highlighting. Block re-rendering is cached in Rust keyed by content hash.
3. **Source / split** — the source pane gains Rust-computed markdown syntax highlighting via a new `highlight_markdown` command (overlay-backdrop technique behind the textarea); side-by-side with the reader pane, scroll-synced.

New Rust commands (each with unit tests): `highlight_markdown(source) -> Vec<HighlightSpan>` (pulldown-cmark offset iterator), `wikilink_complete(root, prefix) -> Vec<Match>` (Rust fuzzy match over the vault index), plus content-hash caching inside `render_blocks`.

Supporting behavior:
- Debounced autosave through the existing `save_file` command.
- Wikilink autocomplete (`[[` trigger) in both editing modes via `wikilink_complete`.
- Incremental milestones to contain risk: (a) `highlight_markdown` command + highlighted source mode; (b) live-block polish + caching; (c) autocomplete + autosave.

## Error handling

- Frontend failures surface as toasts; no blocking dialogs anywhere in the UI.
- Backend command errors return `Result` strings as today and are shown as error toasts.

## Testing

- New Rust commands get unit tests alongside the existing ones (daily notes, graph data, peek).
- Per-milestone verification by running `cargo tauri dev` against the fixture vault.
- Keyboard shortcuts and palette actions added as a checklist to the existing manual test suite doc.

## Out of scope

- Distribution work (signing, auto-update, package managers).
- Performance work for very large files (virtualized rendering).
- Any JS build toolchain.
