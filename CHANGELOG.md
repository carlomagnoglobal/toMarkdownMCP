# Changelog

All notable changes to toMarkdownMCP.

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
