# Getting Started with toMarkdownMCP

**toMarkdownMCP** is a Rust MCP server that converts almost anything — code, HTML, PDF/DOCX/XLSX/PPTX, email, ebooks, feeds, live web pages — into clean Markdown, and layers a full knowledge toolkit on top: **62 MCP tools** covering Chromium-based web capture, Obsidian vault intelligence, RAG chunking/retrieval, and text analytics. The same binary doubles as a terminal Markdown viewer.

## Install (1 minute)

### From source:

```bash
git clone https://github.com/carlomagnoglobal/toMarkdownMCP.git
cd toMarkdownMCP
cargo build --release
```

The binary lands at `./target/release/to_markdown_mcp`.

### Or use the install script (downloads a pre-built binary when available):

```bash
curl -fsSL https://raw.githubusercontent.com/carlomagnoglobal/toMarkdownMCP/main/install.sh | bash
```

## First conversion (30 seconds)

Convert a web page to Markdown over the MCP protocol:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_from_source","arguments":{"source":"https://example.com","file_type":"html"}}}' | ./target/release/to_markdown_mcp
```

## First TUI session

The binary is also an interactive Markdown/vault viewer:

```bash
./target/release/to_markdown_mcp tui /path/to/notes   # a folder or a single .md file
```

Press `?` inside for the full key reference (`j/k` move, `Enter` follows `[[wikilinks]]`, `/` searches, `s` shows text statistics, `q` quits).

## Hook it up to Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS):

```json
{
  "mcpServers": {
    "toMarkdown": {
      "command": "/path/to/toMarkdownMCP/target/release/to_markdown_mcp"
    }
  }
}
```

Restart Claude Desktop — the 62 tools appear automatically. For Claude Code:

```bash
claude mcp add toMarkdown -- /path/to/toMarkdownMCP/target/release/to_markdown_mcp
```

## Where to next

- [QUICK_START.md](QUICK_START.md) — build/run cheatsheet
- [USAGE.md](USAGE.md) — every tool family with examples
- [INSTALL.md](INSTALL.md) — detailed client integration
- [DEPLOYMENT.md](DEPLOYMENT.md) — binaries, source, Docker
- [BROWSER_TOOLS.md](BROWSER_TOOLS.md) · [OBSIDIAN_TOOLS.md](OBSIDIAN_TOOLS.md) · [RAG_TOOLS.md](RAG_TOOLS.md) · [AI_TOOLS.md](AI_TOOLS.md)
