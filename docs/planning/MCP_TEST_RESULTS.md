# MCP Test & Validation Results — v0.1.1

Validation evidence for the first public release. All results reproduced from real runs on 2026-07-10 (macOS, Apple Silicon).

## Unit & integration tests

```
$ cargo test
test result: ok. 282 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out
```

Coverage spans: HTML/markup/document converters, wikilink grammar & vault resolution, frontmatter parsing, task/callout/canvas/dataview parsers, browser HTML sanitization, TUI rendering/wrapping/search, and textmetrics counting + all tokenizer provider paths. (4 ignored tests require network or optional external files.)

## Protocol smoke (stdio JSON-RPC)

```
$ printf '...tools/list...' | ./target/release/to_markdown_mcp
tools: 62
```

- `tools/list` returns 62 tool schemas (captured verbatim in [`MCP_TOOL_SCHEMA.json`](../mcp_functions/MCP_TOOL_SCHEMA.json))
- `tools/call` verified per family:
  - `convert_from_source` on a live URL and on PDF/DOCX/XLSX fixtures ✅
  - `browser_capture_markdown` on a JS-rendered page — full 18.9K-char article captured, metadata frontmatter, no script leakage ✅
  - All 11 `obsidian_*` tools against the fixture vault (`tests/fixtures/mini_vault`) — backlinks via alias forms, ambiguity resolution, dry-run + real rename with link rewriting, canvas conversion, daily-note creation ✅
  - `analyze_text` across all 7 provider paths — exact tiktoken counts, flagged estimates, clean error for a missing tokenizer.json ✅
- Full Claude Desktop handshake replay (numeric ids, initialize → initialized notification → tools/list → ping → tools/call) verified against the release binary; confirmed working in Claude Desktop itself ✅
- Malformed JSON → JSON-RPC parse error (-32700); unknown tool → invalid params (-32602); logs go to stderr only (stdout stays protocol-clean) ✅

## TUI verification

Automated through a PTY + terminal-emulator harness (`developer_examples/capture_tui.py`) at 80×24 and 140×40: rendering (headings, tables, syntax-highlighted fences, callouts), scrolling/wrapping correctness, search + match highlighting, wikilink following, zen/theme/raw toggles, stats popup, live reload on external file edits, clean terminal restore on exit.

## Reproduce locally

```bash
cargo test
printf '%s\n' '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}' \
  | ./target/release/to_markdown_mcp \
  | python3 -c "import json,sys; print(len(json.load(sys.stdin)['result']['tools']),'tools')"
```
