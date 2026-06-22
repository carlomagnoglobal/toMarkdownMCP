# Quick Start Guide

## Build

```bash
cargo build --release
```

Binary will be at: `target/release/to_markdown_mcp`

## Run

```bash
./target/release/to_markdown_mcp
```

The server reads JSON-RPC 2.0 requests from stdin and outputs responses to stdout.

## Examples

### Convert a Python file to Markdown

```bash
echo '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"examples/test.py"}}}' | ./target/release/to_markdown_mcp
```

### Convert Rust code from text

```bash
echo '{"jsonrpc":"2.0","id":"2","method":"tools/call","params":{"name":"convert_text","arguments":{"content":"fn main() { println!(\"Hi\"); }","file_type":"rust","title":"Simple Rust"}}}' | ./target/release/to_markdown_mcp
```

### List available tools

```bash
echo '{"jsonrpc":"2.0","id":"3","method":"tools/list","params":{}}' | ./target/release/to_markdown_mcp
```

## Platform Support

Works on macOS, Linux, and Windows with no platform-specific code.

### Cross-compile
```bash
# Build for Linux on macOS/Windows
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu

# Build for Windows on macOS/Linux  
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

## Supported Languages

Python, Rust, JavaScript/TypeScript, Java, C/C++, Go, Ruby, PHP, Kotlin, Swift, Bash, PowerShell, JSON, YAML, SQL, and 40+ more.

Language is auto-detected from file extension (e.g., `.py` → Python).

## Testing

```bash
cargo test
```

## Integration with Claude

Use with Claude Code or other MCP clients by pointing to the binary:
- Copy binary to `~/.claude/mcp/bin/`
- Configure in `~/.claude/settings.json`
