# toMarkdownMCP Usage Guide

## Quick Start

### Build the Project

```bash
cargo build --release
```

The compiled binary is `target/release/to_markdown_mcp` and serves two roles:
- **MCP server** (default, no arguments): JSON-RPC 2.0 over stdio
- **TUI viewer** (`tui <path>`): interactive terminal Markdown/vault browser

### Talking to the server directly

Every example below is a single JSON line written to stdin; the response comes back on stdout.

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}' | ./target/release/to_markdown_mcp
```

## Tool Families (62 tools)

Call `get_tool_help` (no arguments) for the live catalog, or with `{"tool_name": "..."}` for one tool's parameters.

### 1. Format conversion

`convert_file`, `convert_text`, `convert_from_source` (file/URL/stdin), `batch_convert_files`, `list_directory_files`.

Handles 60+ programming languages, HTML/HTM/MHTML/webarchive, PDF, DOCX/DOC/RTF/ODT, XLSX/XLS/ODS/CSV, PPTX/ODP, EML, EPUB/MOBI, RSS/Atom, and markup (wiki/rst/adoc/org/tex/textile). HTML options: metadata frontmatter, tables, links, images, forms, TOC, and more.

```json
{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"report.pdf"}}}
```

### 2. Browser web capture (real Chromium)

`browser_open_url` → interact (CAPTCHA/login) → `browser_capture_markdown` → `browser_close`. Or one-shot: pass `url` directly to `browser_capture_markdown` for JS-rendered pages. See [BROWSER_TOOLS.md](BROWSER_TOOLS.md).

```json
{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"browser_capture_markdown","arguments":{"url":"https://example.com/spa","wait_seconds":2,"extract_metadata":true}}}
```

### 3. Obsidian vault intelligence

11 `obsidian_*` tools: vault index (broken links, orphans, tags), note lookup by path/stem/alias with embed transclusion, wikilink resolution (`[[target#heading|alias]]`, `#^block`), backlinks, search (tag/alias/field/text), tasks with states and dates, `.canvas` → Markdown, Dataview fields, vault config, templated note creation, and link-rewriting rename (dry-run by default). See [OBSIDIAN_TOOLS.md](OBSIDIAN_TOOLS.md).

### 4. RAG & knowledge

`chunk_markdown`, `extract_chunks_for_rag`, `get_document_outline`, `search_content`, `retrieve_context`, `build_knowledge_index`, `extract_tags/keywords/entities/qa_pairs`, `find_related_notes`, `find_duplicates`, `cluster_documents`, `summarize_document`, readability/language/classification, corpus statistics. Most accept `output_format: "json"`. See [RAG_TOOLS.md](RAG_TOOLS.md) and [SECOND_BRAIN_TOOLS.md](SECOND_BRAIN_TOOLS.md).

### 5. Text analytics — `analyze_text`

Words, characters, spaces, tokens + **sorted frequency tables for words, characters, and tokens**, with modular provider-aware tokenization:

| provider | method | exact? |
|---|---|---|
| `openai` | tiktoken o200k/cl100k by model | ✅ |
| `anthropic` | cl100k proxy | estimate (flagged) |
| `meta`/`qwen`/`deepseek` | HuggingFace tokenizer.json via `tokenizer_file` | ✅ with file, else estimate |
| `grok`, `heuristic` | chars/4 | estimate (flagged) |

```json
{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"analyze_text","arguments":{"file_path":"notes.md","provider":"openai","model":"gpt-4o","top":25}}}
```

Estimated counts always carry an explicit ⚠️ warning and the method used.

### 6. File & vault operations

`read_file`, `create_or_append_file`, `move_or_rename_file`, `delete_file`, `batch_create_notes`, `update_note_properties`, `safe_append_or_replace_section`, `upsert_markdown_table`, `search_files`, `get_file_summary`, `get_recently_modified_files`, `get_vault_statistics`, `extract_active_todos`, and more.

### 7. Claude-backed generation (optional)

`ai_summarize`, `ai_ask` (RAG Q&A with citations), `ai_tag`, `ai_translate`, `ai_classify` — need `ANTHROPIC_API_KEY`; degrade to a setup note without one. See [AI_TOOLS.md](AI_TOOLS.md).

## The TUI viewer

```bash
./target/release/to_markdown_mcp tui /path/to/vault    # or a single .md file
```

Typographic rendering (headings, aligned tables, syntax-highlighted code fences, callout boxes, checkbox glyphs), wikilink following, in-note search, mouse support, zen mode, dark/light themes, live reload, and a text-stats popup.

Key summary (press `?` in-app for the authoritative list):

| Keys | Action |
|---|---|
| `j/k` `↑/↓` · `Space`/`Ctrl+f`/`Ctrl+b` · `Ctrl+d/u` · `g/G` | move · page · half-page · top/bottom |
| `Tab`/`Shift+Tab` · `h`/`l` | switch pane · back / open-follow |
| `Enter` | open file / follow `[[wikilink]]` |
| `/` `n/N` `Esc` | search · next/prev match · cancel |
| `r` · `z` · `T` · `s` | raw view · zen mode · theme · text stats |
| `?` · `q` | help · quit |

Mouse: wheel scrolls, click opens/moves cursor, click a line twice to follow its wikilink.

## Default paths & multiple vaults (`--base-dir`)

Start the server with one or more base directories to avoid absolute paths in every call:

```bash
to_markdown_mcp --base-dir ~/vault --base-dir ~/work-notes
# or: --base-dir ~/vault,~/work-notes
```

- Relative `file_path`/`path`/`vault_path`/... values resolve against the base dirs — the first directory where the path **exists** wins (multi-vault lookup); if it exists nowhere, it resolves against the **first** dir (so newly created files land there).
- `vault_path` and `directory` parameters may be omitted entirely — they default to the first base dir.
- Absolute paths, URLs, and stdin (`-`) are never rewritten. Without the flag, behavior is unchanged.
- `tui` with no path argument also opens the first base dir.

In an MCP client config, pass the flag via `args`:

```json
{"mcpServers": {"toMarkdown": {
  "command": "/path/to/to_markdown_mcp",
  "args": ["--base-dir", "/Users/you/vault"]
}}}
```

## Exit codes & logging

The server logs to **stderr** (stdout is reserved for JSON-RPC). Set `RUST_LOG`-style filtering via the built-in `info` default.
