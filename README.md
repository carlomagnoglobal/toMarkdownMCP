# toMarkdownMCP

A Model Context Protocol (MCP) server written in Rust that converts plaintext and code files to Markdown format. Cross-platform compatible with Windows, Linux, and macOS.

## Features

- **Convert code files** to Markdown format from multiple sources:
  - File paths (local files)
  - HTTP/HTTPS URLs
  - stdin
  - Directory scanning with automatic code file detection
- **Auto-detect 60+ programming languages** from file extensions and filenames
- **Line numbers** support for better code readability
- **Code syntax highlighting** with proper Markdown code blocks
- **Explicit language specification** for extension-less files
- **Cross-platform** support (Windows, Linux, macOS)
- **JSON-RPC 2.0** MCP protocol implementation
- **Zero external MCP dependencies** - pure Rust implementation

## Supported Languages (60+)

The server auto-detects and properly formats code for:

- **Web**: HTML, CSS, SCSS, Sass, Less, JavaScript, JSX, TypeScript, TSX, Vue, Svelte, Astro
- **Server-side**: Python, Ruby, PHP, Java, C#, C++, C, Rust, Go, Kotlin, Swift, Objective-C, Scala, Groovy, VB.NET, ASP
- **Scripting**: Bash, PowerShell, Batch, Fish, Perl, AWK, Sed
- **Data/Config**: JSON, YAML, XML, TOML, INI, Properties, SQL, GraphQL, Protocol Buffers
- **Markup/Docs**: Markdown, ReStructuredText, LaTeX, AsciiDoc
- **Build**: Dockerfile, Makefile, CMake, Gradle, Ninja
- **Functional**: Lisp, Scheme, Racket, Clojure, Elixir, Erlang, Haskell, OCaml, F#, Zig
- **Data Science**: R, Julia, Python (with RMarkdown)
- **Other**: Lua, Nim, Dart, Vim, and more...

**Filename-Based Detection:** Also detects Dockerfile, Makefile, .bashrc, .gitignore, package.json, Cargo.toml, etc.

## Building

### Prerequisites
- Rust 1.70 or later (install from https://rustup.rs/)

### Build for your platform
```bash
cargo build --release
```

The binary will be in `target/release/to_markdown_mcp` (or `.exe` on Windows)

### Cross-compile
```bash
# Build for Linux from macOS
cargo build --release --target x86_64-unknown-linux-gnu

# Build for Windows from macOS
cargo build --release --target x86_64-pc-windows-gnu

# Build for macOS from Linux
cargo build --release --target x86_64-apple-darwin
```

## Usage

### Running the MCP Server

```bash
./target/release/to_markdown_mcp
```

The server reads JSON-RPC 2.0 requests from stdin and writes responses to stdout.

### Available Tools

#### 1. `convert_file`
Converts a file to Markdown format.

**Parameters:**
- `file_path` (string, required): Path to the file to convert
- `include_filename` (boolean, optional): Include filename as heading (default: true)
- `file_type` (string, optional): Explicitly specify language (overrides detection)
- `add_line_numbers` (boolean, optional): Add line numbers to code block (default: false)

**Example:**
```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "tools/call",
  "params": {
    "name": "convert_file",
    "arguments": {
      "file_path": "/path/to/script.py",
      "add_line_numbers": true
    }
  }
}
```

#### 2. `convert_text`
Converts plain text content to Markdown format.

**Parameters:**
- `content` (string, required): The text content to convert
- `file_type` (string, optional): Programming language identifier (e.g., 'rust', 'python', 'javascript')
- `title` (string, optional): Title for the Markdown document
- `add_line_numbers` (boolean, optional): Add line numbers to code block (default: false)

**Example:**
```json
{
  "jsonrpc": "2.0",
  "id": "2",
  "method": "tools/call",
  "params": {
    "name": "convert_text",
    "arguments": {
      "content": "fn main() { println!(\"Hello\"); }",
      "file_type": "rust",
      "title": "Hello World in Rust",
      "add_line_numbers": true
    }
  }
}
```

#### 3. `convert_from_source` (NEW)
Converts code from various sources (file, URL, stdin) to Markdown format.

**Parameters:**
- `source` (string, required): File path, HTTP/HTTPS URL, or `-` for stdin
- `file_type` (string, optional): Explicitly specify language (overrides detection)
- `title` (string, optional): Title for the Markdown document
- `add_line_numbers` (boolean, optional): Add line numbers to code block (default: false)

**Example - From URL:**
```json
{
  "jsonrpc": "2.0",
  "id": "3",
  "method": "tools/call",
  "params": {
    "name": "convert_from_source",
    "arguments": {
      "source": "https://raw.githubusercontent.com/user/repo/main/src/main.rs",
      "add_line_numbers": true
    }
  }
}
```

**Example - From stdin:**
```json
{
  "jsonrpc": "2.0",
  "id": "4",
  "method": "tools/call",
  "params": {
    "name": "convert_from_source",
    "arguments": {
      "source": "-",
      "file_type": "python"
    }
  }
}
```

#### 4. `list_directory_files` (NEW)
Lists all code files in a directory.

**Parameters:**
- `directory` (string, required): Directory path to scan
- `recursive` (boolean, optional): Recursively scan subdirectories (default: true)

**Example:**
```json
{
  "jsonrpc": "2.0",
  "id": "5",
  "method": "tools/call",
  "params": {
    "name": "list_directory_files",
    "arguments": {
      "directory": "src",
      "recursive": true
    }
  }
}
```

**Response:**
```
# Code Files in: src

Found 12 files:

- `src/main.rs`
- `src/lib.rs`
- `src/utils.rs`
...
```

## Testing

Run the test suite:

```bash
cargo test
```

### Example Test Files

Create test files to try it out:

```bash
# Create a test Python file
cat > test.py << 'EOF'
def hello(name):
    print(f"Hello, {name}!")

if __name__ == "__main__":
    hello("World")
EOF

# Create a test Rust file
cat > test.rs << 'EOF'
fn main() {
    println!("Hello, World!");
}
EOF

# Create a test text file
cat > test.txt << 'EOF'
This is a simple text file.
It can contain any plaintext content.
EOF
```

## Protocol

The server implements the MCP (Model Context Protocol) specification using JSON-RPC 2.0:

### Initialization
The client should send an initialization request. The server responds with capabilities.

### Tool Discovery
Request: `{"method": "tools/list"}`
Response: Lists all available tools with their descriptions and input schemas

### Tool Execution
Request: `{"method": "tools/call", "params": {"name": "...", "arguments": {...}}}`
Response: Execution result with converted Markdown content

## Architecture

- **main.rs**: MCP server implementation and request handling
- **converter.rs**: Markdown conversion logic
- **file_type.rs**: Programming language detection from file extensions
- **error.rs**: Custom error types

## Performance

- Minimal dependencies for fast compilation
- Single-threaded async I/O for efficient stdio handling
- Direct file reading without intermediate processing

## License

MIT

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test`
- No clippy warnings: `cargo clippy`
- Code is formatted: `cargo fmt`
