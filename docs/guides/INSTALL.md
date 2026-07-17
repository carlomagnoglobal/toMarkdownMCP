# Installation & Setup

## Quick Start — Claude Desktop Integration

### Step 1: Build the binary

```sh
cargo build --release
```

The MCP binary will be at `./target/release/to_markdown_mcp`.

Prerequisites: Rust 1.88+ (`curl https://sh.rustup.rs -sSf | sh`). The browser tools additionally need Google Chrome, Chromium, or Edge installed (or set the `CHROME` env var to the executable); everything else is self-contained.

### Step 2: Register the server

**Claude Desktop** — edit the config file:

| OS | Path |
|----|------|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |

```json
{
  "mcpServers": {
    "toMarkdown": {
      "command": "/path/to/toMarkdownMCP/target/release/to_markdown_mcp",
      "args": ["--base-dir", "/path/to/your/vault"]
    }
  }
}
```

Use the **absolute** path to the binary. Restart Claude Desktop.

The optional `--base-dir` flag sets your default vault/working directory once, so tool
calls can use relative paths (`"file_path": "notes/todo.md"`) or omit `vault_path`
entirely. Repeat the flag (or comma-separate) for multiple vaults — relative paths pick
the first vault where the file exists; new files are created in the first one:

```json
"args": ["--base-dir", "/Users/you/vault", "--base-dir", "/Users/you/work-notes"]
```

**Claude Code**:

```bash
claude mcp add toMarkdown -- /path/to/toMarkdownMCP/target/release/to_markdown_mcp --base-dir /path/to/your/vault
```

**Any other MCP client**: the server speaks JSON-RPC 2.0 over stdio (newline-delimited). Point the client's `command` at the binary with no arguments.

### Step 3: Verify

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}' | ./target/release/to_markdown_mcp | head -c 300
```

You should see a JSON response listing tools (62 in total). In Claude Desktop, ask: *"use the get_tool_help tool"* — it returns the full catalog.

## Optional pieces

- **Browser capture** (`browser_open_url`, `browser_capture_markdown`): needs a local Chromium-family browser. First use may take a few seconds while a fresh browser profile starts.
- **AI tools** (`ai_summarize`, `ai_ask`, …): set `ANTHROPIC_API_KEY` in the environment of the MCP client. Without a key these tools return a setup note instead of failing.
- **Exact Llama/Qwen/DeepSeek token counts** (`analyze_text`): download the model's `tokenizer.json` from HuggingFace and pass its path via the `tokenizer_file` parameter.

## The TUI viewer

No configuration needed:

```bash
./target/release/to_markdown_mcp tui /path/to/vault-or-file
./target/release/to_markdown_mcp --help
```

## Troubleshooting

| Symptom | Fix |
|---|---|
| Client says server failed to start | Check the binary path is absolute and executable; run it manually and paste a `tools/list` request |
| Browser tools error "Failed to start Chromium" | Install Chrome/Chromium or `export CHROME=/path/to/chrome` |
| Build fails with `ENOSPC` | Free disk space; `cargo clean` removes regenerable artifacts |
| Tokens look off for Claude | Anthropic counts are labeled estimates — see [USAGE.md](USAGE.md#analyze_text) |
