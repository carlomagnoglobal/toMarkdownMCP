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

## CLI (no MCP client needed)

```bash
to_markdown_mcp convert report.pdf                    # any supported format → Markdown on stdout
to_markdown_mcp convert https://example.com -o page.md
to_markdown_mcp convert script.py --line-numbers --type python
to_markdown_mcp batch a.md b.pdf c.docx -o combined.md   # up to 10 files
to_markdown_mcp search "kubernetes" --dir ./notes
to_markdown_mcp tools                                  # list all 62 MCP tools
to_markdown_mcp tools convert_file                     # detailed help for one tool
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
