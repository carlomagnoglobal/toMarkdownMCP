# toMarkdownMCP Development Roadmap

Multi-phase roadmap as of July 2026. Each phase is independently shippable: green `cargo test`, docs updated, committed and pushed to main. Publishing/registry submissions are **deferred** and tracked separately in [docs/deployment/](../deployment/).

Current baseline: v0.1.1 on crates.io — 62 MCP tools ([FUNCTIONS_REFERENCE.md](../mcp_functions/FUNCTIONS_REFERENCE.md)), TUI viewer, 282 tests passing.

## Phase 0 — Roadmap refresh ✅

- [x] Update [PLANNED_ENHANCEMENTS.md](PLANNED_ENHANCEMENTS.md) to mark shipped items
- [x] Create this roadmap and link it from the README

## Phase 1 — CLI support ✅ (code complete; ships as v0.2.0)

Make the binary usable directly from the terminal, not only as an MCP server.

- [x] Proper CLI via `clap` (derive), preserving existing behavior: no args → MCP server; `tui [PATH]`; `--base-dir`
- [x] `convert <SOURCE> [-o out.md] [--type LANG] [--line-numbers] [--title T]` — file/URL/stdin → Markdown (wraps `convert_from_source` logic)
- [x] `batch <FILES>... [-o out.md]` — combined document for up to 10 files (wraps `batch_convert_files`)
- [x] `search <QUERY> --dir <DIR>` — wraps `search_content`
- [x] `tools [TOOL_NAME]` — tool catalog / per-tool help (reuses `get_tool_help`; detailed help now falls back to the tools/list schema for every tool)
- [x] CLI arg-parsing unit tests + CLI integration tests on the fixture vault
- [x] Docs: USAGE.md, QUICK_START.md, README

## Phase 2 — MCP protocol depth ✅

- [x] `resources/list` + `resources/read`: expose `--base-dir` vault files as MCP resources (`file://` URIs, capped at 1000; reads confined to the base dirs; non-Markdown formats converted on read)
- [x] `prompts/list` + `prompts/get`: `summarize_note`, `ingest_url`, `vault_health` templates
- [x] Declare `resources` and `prompts` capabilities in the `initialize` response
- [x] `MCP_TOOL_SCHEMA.json` unchanged (tool list did not change)

## Phase 3 — Real embeddings RAG ✅

- [x] `Embedder` trait (`src/embeddings.rs`) with a fastembed/ONNX backend (all-MiniLM-L6-v2, behind the opt-in `embeddings` cargo feature) and an always-available hashed-vector fallback
- [x] Persistent per-directory vector index (`.tomarkdown/embeddings_index.json`) with incremental re-index by mtime and model-change invalidation
- [x] Wired into `retrieve_context`, `find_related_notes`, `find_duplicates`, `cluster_documents` behind an opt-in `embeddings: true` parameter (no breaking schema changes; `MCP_TOOL_SCHEMA.json` regenerated)

## Phase 4 — Hardening & quality ✅

- [x] Large-file gate for >10MB files: plain text/code streams through a single pre-sized buffer; structured formats (HTML/docs/markup) are refused with guidance and a `max_bytes` override on `convert_file`; same gate protects the RAG directory scans
- [x] `cargo clippy --all-targets -D warnings` clean; clippy enforced in CI (rustfmt deliberately not enforced — the pre-existing style diverges and a repo-wide reformat would bury history)
- [x] End-to-end JSON-RPC integration tests per tool family (`tests/jsonrpc.rs`), including resources/prompts and error-shape checks
- [x] Error audit: missing/invalid arguments now return `-32602 Invalid params` naming the parameter; execution failures return `-32603` with the real cause instead of a generic "Internal error"

## Phases 5–9 — GUI application

Desktop GUI with functionality on par with **Obsidian, Typora, MacMD Viewer, and Marked 2**. Tauri app in a new `gui/` crate (repo becomes a Cargo workspace; shared logic extracted to a library crate both binaries link). GUI MVP (Phase 5) ships as v0.3.0.

### Phase 5 — Foundation & viewer (MacMD Viewer parity) ✅ (packaging pending)

- [x] Cargo workspace restructuring: shared `src/lib.rs` + `src/pipeline.rs`; `default-members` keeps plain `cargo build`/`test` on the MCP crate
- [x] Tauri viewer crate (`gui/`, run with `cargo run -p to_markdown_gui`): file-tree sidebar, rendered Markdown pane (pulldown-cmark), OS light/dark theme — see [docs/gui/GUI.md](../gui/GUI.md)
- [x] Open any supported format (PDF/DOCX/HTML/... via the shared conversion pipeline; code files render as fenced blocks with detected language)
- [x] Drag-and-drop (files and folders), native open dialogs, persisted recent files
- [ ] macOS `.app` packaging (`bundle.active` currently false; needs tauri-cli + icon set); Linux/Windows CI builds — deferred to the Phase 9 distribution pass

### Phase 6 — Live preview & watching (Marked 2 parity) ✅

- [x] File and folder watching with debounced re-render (`notify` crate; tree refresh on create/remove, file reload on save)
- [x] Scroll-position preservation across live reloads (ratio-based)
- [x] Clickable TOC sidebar built from the rendered headings (JS-side; simpler than round-tripping through `toc_generator.rs`)
- [x] Export standalone styled HTML (theme + user CSS inlined), Print/save-as-PDF via the system dialog, copy-as-rich-text
- [x] Bundled themes (System/Light/Dark/Sepia) + user CSS file support, persisted
- [x] Stats footer: words/chars/read-time

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
