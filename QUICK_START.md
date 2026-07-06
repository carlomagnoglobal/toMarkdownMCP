# Quick Start Guide

## Build

```bash
cargo build --release
```

Binary will be at: `target/release/to_markdown_mcp`

## Run

```bash
./target/release/to_markdown_mcp            # MCP server (JSON-RPC 2.0 over stdio)
./target/release/to_markdown_mcp tui .      # interactive terminal Markdown viewer
./target/release/to_markdown_mcp --help
```

## Examples

### List the 62 available tools

```bash
echo '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}' | ./target/release/to_markdown_mcp
```

### Convert a file (code, PDF, DOCX, XLSX, EPUB, …) to Markdown

```bash
echo '{"jsonrpc":"2.0","id":"2","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"report.pdf"}}}' | ./target/release/to_markdown_mcp
```

### Convert a live web page (real Chromium, handles JS)

```bash
echo '{"jsonrpc":"2.0","id":"3","method":"tools/call","params":{"name":"browser_capture_markdown","arguments":{"url":"https://example.com","extract_metadata":true}}}' | ./target/release/to_markdown_mcp
```

### Index an Obsidian vault

```bash
echo '{"jsonrpc":"2.0","id":"4","method":"tools/call","params":{"name":"obsidian_vault_index","arguments":{"vault_path":"/path/to/vault","include_orphans":true}}}' | ./target/release/to_markdown_mcp
```

### Analyze text metrics & tokens

```bash
echo '{"jsonrpc":"2.0","id":"5","method":"tools/call","params":{"name":"analyze_text","arguments":{"content":"hello hello world","provider":"openai","model":"gpt-4o"}}}' | ./target/release/to_markdown_mcp
```

## Platform Support

macOS, Linux, and Windows. Pre-built binaries on [GitHub Releases](https://github.com/carlomagnoglobal/toMarkdownMCP/releases); cross-compile with the usual `cargo build --release --target ...`.

## Testing

```bash
cargo test        # 272 tests
```

## Integration with Claude

**Claude Code**:
```bash
claude mcp add toMarkdown -- /path/to/toMarkdownMCP/target/release/to_markdown_mcp
```

**Claude Desktop** and other clients: see [INSTALL.md](INSTALL.md). Full tool documentation: [USAGE.md](USAGE.md).
