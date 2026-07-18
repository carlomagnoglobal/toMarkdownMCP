# Changelog

All notable changes to toMarkdownMCP.

## Unreleased

### Added — GUI: drop into notes and link localization

- **Drop into notes**: Drag files, images, or URLs into the note editor to add content. Image files are instantly embedded into the vault's attachment folder; other files and URLs show a four-action dialog (Copy into vault & link / Link original location / Convert to new linked note / Convert inline, with oversized content requiring confirmation). Multi-file drops apply a single action to all files. Safari/browser image drags download and embed, with a link fallback if offline.
- **Link localization**: Right-click external links or remote images in the reader to "Store in vault" (download and rewrite the link) or "Convert to markdown note" (create an Imports/ note and rewrite as a wikilink). Palette command "Localize External Links…" scans the entire note for external targets, lists them with per-item actions (images default to Store, links default to Skip), and processes sequentially with failure tolerance.

## gui-v0.3.0 — 2026-07-18 (GUI-only release)

toMarkdown Viewer 0.3.0. The `to_markdown_mcp` crate is unchanged (stays 0.2.0 on crates.io).

### Added — GUI: reading polish

- **Image lightbox**: click any image in the reader to zoom; Esc or click closes
- **Reading-progress bar** at the top of the reader pane
- **Keyboard-shortcut cheat sheet**: press `?` for a complete overlay of every binding
- **Text sizing**: Cmd/Ctrl+= / − / 0 (clamped 11–24 px) plus a line-height setting, both persisted
- **Print polish**: clean page output with no app chrome and no mid-block page breaks in code/tables

### Added — GUI: Rust-first hybrid editing

- **Rust-computed syntax highlighting** in split-source mode (backdrop overlay): headings, bold, italic, inline code, wikilinks, links, blockquotes, list markers, fenced code blocks, and frontmatter rendered live as you type
- **Live-block highlighting**: in Typora-style live mode, the active editing block is highlighted for better focus
- **`[[` wikilink autocomplete** (shared between source and live modes): ranked results by title prefix, substring, then alias; navigate with Arrow Up/Down, accept with Enter/Tab, dismiss with Escape
- **Silent autosave**: 1.2-second debounce after typing stops; saves atomically without toast notifications in split mode
- **Proportional scroll sync**: editor ↔ preview in split mode stay aligned as you scroll either pane
- **Render-block caching**: performance optimization for long notes and frequent mode switches

## gui-v0.2.0 — 2026-07-16 (GUI-only release)

toMarkdown Viewer 0.2.0. The `to_markdown_mcp` crate is unchanged (stays 0.2.0 on crates.io).

### Added

- **Import / convert bar**: Import… toolbar panel plus drag-and-drop conversion of any pipeline-supported file (PDF, DOCX, XLSX, PPTX, EPUB, EML, HTML, CSV, AsciiDoc, Org, LaTeX, …) and pasted URLs to Markdown; auto-saves into the vault's `Imports/` folder (deduped filenames) and opens in a new tab, or Save As when no vault is open
- **Text analysis**: tabbed overlay (Summary / Words / Characters / Tokens) with exact OpenAI token counts, an Anthropic/Claude estimate, and complete frequency tables (rank, item, count, share %); opened from the clickable status-bar counter or the palette; the status bar shows a live token estimate
- **Feedback**: toast notifications, busy spinners on long actions, human-readable error messages, empty-state hints in Search/Tags/Tasks/backlinks panes
- **Accessibility**: ARIA dialogs with focus trap + restore, keyboard-activatable tree/tabs/results with visible focus outlines
- **Zen mode shortcut** (Cmd+Shift+Z; also hides the tab bar); always-visible themed scrollbars

### Fixed

- Dark-theme syntax highlighting was unreadable for JSON (and other deep-selector grammars): light-theme CSS rules with very high specificity leaked into dark mode; light rules are now hard-scoped per theme
- Import Save As dialog could deadlock the app (blocking dialog on the main thread)
- Note content (frontmatter, tags, backlink context, search snippets) is HTML-escaped in the sidebar; theme colors (errors, callouts, highlights, shadows) are token-driven across all four themes

## v0.2.0 — 2026-07-16

### Added — CLI

The binary is now a standalone CLI alongside its MCP-server and TUI roles (argument parsing moved to clap; `--base-dir`, `tui`, and no-args server mode unchanged):

- `convert <SOURCE> [-o FILE] [--type LANG] [--line-numbers] [--title T]` — file, URL, or stdin → Markdown
- `batch <FILES>... [-o FILE]` — combined document for up to 10 files
- `search <QUERY> --dir <DIR>` — ranked full-text search
- `tools [TOOL_NAME]` — tool catalog / per-tool help. Detailed help now works for **all 62 tools**: unknown names fall back to rendering the tool's schema (previously most tools answered "Unknown tool")

### Added — MCP protocol depth

- `resources/list` / `resources/read`: files under the `--base-dir` vault(s) exposed as `file://` resources (capped at 1000; reads confined to the base dirs; non-Markdown formats converted to Markdown on read)
- `prompts/list` / `prompts/get`: `summarize_note`, `ingest_url`, `vault_health` templates
- `initialize` now declares `resources` and `prompts` capabilities

### Added — vector embeddings (opt-in)

- `retrieve_context`, `find_related_notes`, `find_duplicates`, `cluster_documents` accept `embeddings: true` for vector-similarity ranking
- Real sentence embeddings (all-MiniLM-L6-v2 via fastembed/ONNX) behind the optional `embeddings` cargo feature; deterministic hashed-vector fallback otherwise, so the flag never fails outright
- Chunk vectors persist per directory in `.tomarkdown/embeddings_index.json`, re-embedded incrementally by mtime
- `find_duplicates` gains `min_similarity` (cosine, default 0.9) for embeddings mode

### Added — desktop GUI (in-repo, not part of the published crate)

`gui/` contains **toMarkdown Viewer**, a Tauri desktop app sharing this crate as a library: file-tree/vault browsing, live preview with file watching, Obsidian-grade navigation (wikilinks, backlinks, tags, graph, quick switcher), Typora-style in-place block editing, callouts/math/Mermaid/syntax-highlighted rendering, AI actions, DOCX/RTF/HTML export, native macOS menu and `.md` file associations. Build with `cargo run -p to_markdown_gui`; bundle with `cargo tauri build` from `gui/`. See `docs/gui/GUI.md`.

### Changed / hardening

- Files over 10 MB: plain text/code streams through a single pre-sized buffer; structured formats (HTML/documents/markup) are refused with guidance and a `max_bytes` override on `convert_file`; the same gate protects RAG directory scans
- JSON-RPC error taxonomy: missing/invalid arguments return `-32602 Invalid params` naming the parameter; execution failures return `-32603` with the real cause (previously a generic "Internal error")
- Crate restructured as a library + binary (Cargo workspace); `cargo clippy -D warnings` clean and enforced in CI; end-to-end JSON-RPC integration tests per tool family

## v0.1.1 — 2026-07-10

### Fixed — MCP protocol compliance (critical)

v0.1.0 could not connect to Claude Desktop or other strict MCP clients:

- Implemented the `initialize` handshake (returns protocolVersion echoing the client's, tools capability, serverInfo) and `ping`
- Request `id` now accepts numbers as well as strings (clients send `id: 0`); responses echo it verbatim; parse errors respond with `id: null`
- JSON-RPC notifications (e.g. `notifications/initialized`) are consumed silently instead of receiving Method-not-found errors
- Responses no longer emit a `null` for the unused `result`/`error` key — strict client schemas reject messages carrying both

### Added

- `--base-dir DIR` server flag (repeatable / comma-separated for multiple vaults): relative tool paths resolve against the configured directories (first existing match wins; new files go to the first dir), `vault_path`/`directory` parameters become optional (default to the first dir), and `tui` with no path opens the first dir. Fully backward compatible when the flag is absent.

## v0.1.0 — 2026-07-06

First public release. 🎉

### MCP server — 62 tools over JSON-RPC 2.0 stdio

- **Format conversion**: 60+ programming languages; HTML/HTM/MHTML/webarchive with rich extraction options (metadata frontmatter, tables, links, images, forms, comments, definition lists, TOC, code-language detection); documents (PDF, DOCX, DOC, RTF, ODT), spreadsheets (XLSX, XLS, ODS, CSV), presentations (PPTX, ODP), email (EML), ebooks (EPUB, MOBI), feeds (RSS/Atom), and markup (MediaWiki, RST, AsciiDoc, Org, LaTeX, Textile); sources: files, URLs, stdin, directories.
- **Browser web capture** (Chromium via chromiumoxide): `browser_open_url` / `browser_capture_markdown` / `browser_close` with human-in-the-loop support — open a visible window, solve CAPTCHAs or log in, then capture the rendered DOM as Markdown. One-shot headless mode for JS-rendered pages.
- **Obsidian vault suite** (11 tools): full wikilink grammar (`[[target#heading|alias]]`, `#^block`, embeds, code-fence awareness), shortest-path link resolution, backlinks with context, vault index (tags, aliases, broken/ambiguous links, orphans), search (tag/alias/frontmatter-field/text), tasks with all checkbox states + Tasks-plugin dates, `.canvas` → Markdown, Dataview inline fields, `.obsidian` config, templated note/daily-note creation, and link-rewriting rename (dry-run default).
- **RAG & knowledge toolkit**: heading-aware chunking, retrieval with budgeting and citations, knowledge index, outlines, ranked content search, tags/keywords/entities/Q&A extraction, related-notes similarity, near-duplicate detection, clustering, readability and language detection, corpus statistics.
- **Text analytics**: `analyze_text` — words/characters/spaces/tokens plus sorted frequency tables for words, characters, and tokens; modular provider-aware tokenization (OpenAI tiktoken exact; Anthropic cl100k proxy clearly flagged as estimate; Meta/Qwen/DeepSeek exact via HuggingFace tokenizer.json; Grok/heuristic estimates flagged).
- **File & vault operations**: token-efficient file summaries, batch conversion, search with context, frontmatter property editing, section-safe appends, table upserts, TODO extraction, and more.
- **Claude-backed generation** (optional, `ANTHROPIC_API_KEY`): abstractive summaries, RAG Q&A with citations, tagging, translation, classification.

### Terminal UI (`to_markdown_mcp tui <path>`)

- Typographic Markdown rendering: styled headings (ATX + setext), boxed frontmatter properties, re-aligned pipe tables, syntax-highlighted code fences (syntect), callout boxes with icons, checkbox glyphs, bullet/number lists with hanging indents, link/URL cleanup.
- Vault-aware navigation: file tree grouped by directory, `[[wikilink]]` following with Obsidian resolution, backlink/tag title bar, breadcrumbs.
- Interaction: in-note search with match highlighting (`/`, `n/N`), mouse support (scroll, click-to-open, click-twice-to-follow), zen mode (`z`), dark/light themes (`T`), raw-source toggle (`r`), text-stats popup (`s`), help overlay (`?`), live reload on external edits, scrollbar, reading-width cap.

### Quality

- 272 unit/integration tests; fixture Obsidian vault; pyte-based TUI verification harness (`developer_examples/`).
- Logs to stderr (stdout reserved for JSON-RPC).
