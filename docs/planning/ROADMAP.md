# toMarkdownMCP Development Roadmap

Multi-phase roadmap as of July 2026. Each phase is independently shippable: green `cargo test`, docs updated, committed and pushed to main. Publishing/registry submissions are **deferred** and tracked separately in [docs/deployment/](../deployment/).

Current baseline: v0.1.1 on crates.io — 62 MCP tools ([FUNCTIONS_REFERENCE.md](../mcp_functions/FUNCTIONS_REFERENCE.md)), TUI viewer, 282 tests passing.

## Phase 0 — Roadmap refresh ✅

- [x] Update [PLANNED_ENHANCEMENTS.md](PLANNED_ENHANCEMENTS.md) to mark shipped items
- [x] Create this roadmap and link it from the README

## Phase 1 — CLI support (ships as v0.2.0)

Make the binary usable directly from the terminal, not only as an MCP server.

- [ ] Proper CLI via `clap` (derive), preserving existing behavior: no args → MCP server; `tui [PATH]`; `--base-dir`
- [ ] `convert <SOURCE> [-o out.md] [--type LANG] [--line-numbers]` — file/URL/stdin → Markdown (wraps `convert_from_source` logic)
- [ ] `batch <DIR> [-o outdir]` — wraps batch conversion
- [ ] `search <QUERY> --dir <DIR>` — wraps `search_content`
- [ ] `tools` — print the tool catalog (reuses `get_tool_help`)
- [ ] CLI arg-parsing unit tests + `convert` integration test on a fixture
- [ ] Docs: USAGE.md, QUICK_START.md, README

## Phase 2 — MCP protocol depth

- [ ] `resources/list` + `resources/read`: expose `--base-dir` vault files as MCP resources
- [ ] `prompts/list` + `prompts/get`: prompt templates (summarize note, ingest URL, ...)
- [ ] Declare new capabilities in the `initialize` response
- [ ] Regenerate `MCP_TOOL_SCHEMA.json` if the tool list changes

## Phase 3 — Real embeddings RAG

- [ ] Embeddings trait with a local ONNX backend (`fastembed`) and TF-vector fallback
- [ ] Persistent vector index on disk with incremental re-index by mtime
- [ ] Wire into `retrieve_context`, `find_related_notes`, `find_duplicates`, `cluster_documents` behind an opt-in `embeddings: true` parameter (no breaking schema changes)

## Phase 4 — Hardening & quality

- [ ] Streaming/chunked conversion for >10MB files (see [PLANNED_ENHANCEMENTS.md](PLANNED_ENHANCEMENTS.md))
- [ ] `cargo clippy --all-targets -D warnings` clean; fmt + clippy in CI
- [ ] End-to-end JSON-RPC integration test per tool family
- [ ] Error-message audit: consistent JSON-RPC error codes/messages

## Phases 5–9 — GUI application

Desktop GUI with functionality on par with **Obsidian, Typora, MacMD Viewer, and Marked 2**. Tauri app in a new `gui/` crate (repo becomes a Cargo workspace; shared logic extracted to a library crate both binaries link). GUI MVP (Phase 5) ships as v0.3.0.

### Phase 5 — Foundation & viewer (MacMD Viewer parity)

- [ ] Cargo workspace restructuring; shared `lib.rs`
- [ ] File-tree sidebar, rendered Markdown pane, OS light/dark theme
- [ ] Open any supported format (PDF/DOCX/HTML/... via conversion)
- [ ] Drag-and-drop / file association, recent files
- [ ] macOS `.app` packaging; Linux/Windows best-effort in CI

### Phase 6 — Live preview & watching (Marked 2 parity)

- [ ] File/folder watching with instant re-render (`notify` crate)
- [ ] Scroll preservation and synced scrolling
- [ ] Clickable TOC sidebar (reuses `toc_generator.rs` / `heading_analyzer.rs`)
- [ ] Export styled HTML and PDF; copy-as-rich-text
- [ ] Bundled CSS themes + user CSS support
- [ ] Stats footer: words/chars/read-time (reuses `textmetrics.rs`)

### Phase 7 — Vault navigation & knowledge (Obsidian parity, read side)

- [ ] Wikilink following with embeds/transclusion (reuses `src/obsidian/`)
- [ ] Backlinks panel, tag browser, properties panel, Dataview fields
- [ ] Vault-wide search: full text / tag / alias / field
- [ ] Interactive graph view (global + local per note)
- [ ] Tasks view; `.canvas` rendering
- [ ] Quick switcher (fuzzy open by title/alias)

### Phase 8 — Editing (Typora / Obsidian parity, write side)

- [ ] Typora-style live WYSIWYG editing (Milkdown or CodeMirror 6 live preview)
- [ ] Autosave, undo/redo, find-and-replace
- [ ] Table editor, click-to-toggle tasks, wikilink & tag autocomplete, paste-image-into-vault
- [ ] New note from template, rename with link rewriting, frontmatter property editor

### Phase 9 — GUI intelligence & polish

- [ ] Related-notes pane, semantic search (with Phase 3 embeddings), ai_* actions
- [ ] Command palette + customizable shortcuts
- [ ] Settings UI; persisted preferences
- [ ] Large-vault performance pass; accessibility check
- [ ] Signed/notarized macOS build (certs permitting); release artifacts for all platforms

## Deferred

- mcp.so / Docker Hub MCP Registry submissions — see [PUBLISH_TO_REGISTRIES.md](../deployment/PUBLISH_TO_REGISTRIES.md)
