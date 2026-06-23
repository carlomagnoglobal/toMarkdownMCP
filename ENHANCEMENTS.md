# v0.2.0 Enhancements - Code File Source Support

## New Features

### 1. **Line Numbers Support**
Add line numbers to all code blocks with the `add_line_numbers` parameter.

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "script.py",
    "add_line_numbers": true
  }
}
```

Output will include numbered lines:
```
   1 | def hello():
   2 |     print("Hello")
```

### 2. **Explicit Language Specification**
Override auto-detection by specifying file type explicitly.

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "Makefile",
    "file_type": "makefile"
  }
}
```

Useful for extension-less files or files with non-standard extensions.

### 3. **Multiple File Source Support**
New `convert_from_source` tool accepts multiple source types:

- **File paths**: `"source": "path/to/file.py"`
- **HTTP/HTTPS URLs**: `"source": "https://github.com/raw/file.rs"`
- **stdin**: `"source": "-"` or `"source": "stdin"`

```json
{
  "name": "convert_from_source",
  "arguments": {
    "source": "https://example.com/script.py",
    "add_line_numbers": true
  }
}
```

### 4. **Directory Scanning**
List all code files in a directory with `list_directory_files`.

```json
{
  "name": "list_directory_files",
  "arguments": {
    "directory": "src",
    "recursive": true
  }
}
```

Returns:
```
# Code Files in: src

Found 15 files:

- `src/main.rs`
- `src/lib.rs`
- `src/utils.rs`
...
```

### 5. **Enhanced Language Detection** (60+ languages)

**Added Support For:**
- F# (`.fs`, `.fsx`, `.fsi`)
- Julia (`.jl`)
- Zig (`.zig`)
- Nim (`.nim`)
- Dart (`.dart`)
- Astro (`.astro`)
- Svelte (`.svelte`)
- Forth (`.fth`)
- Haskell (`.hs`)
- OCaml (`.ml`, `.mli`)
- Clojure (`.clj`, `.cljs`)
- Elixir (`.ex`, `.exs`)
- Erlang (`.erl`, `.hrl`)
- And more...

**Filename-Based Detection (for extension-less files):**
- `Dockerfile` → dockerfile
- `Makefile` → makefile
- `.gitignore` → properties
- `.bashrc`, `.zshrc` → bash
- `Gemfile`, `Rakefile` → ruby
- `package.json`, `tsconfig.json` → json
- `Cargo.toml` → toml
- And more common config files...

### 6. **Intelligent File Scanning**
`list_directory_files` automatically:
- Detects code files by extension and name
- Recursively scans subdirectories (configurable)
- Excludes common non-code directories:
  - `node_modules`, `target`, `.git`
  - `__pycache__`, `.venv`, `dist`, `build`
  - `.vscode`, `.idea`, `vendor`, etc.
- Skips hidden files (starting with `.`)

## Tool Signatures

### convert_file
```json
{
  "file_path": "string (required) - Path to file",
  "include_filename": "boolean (default: true) - Show filename as heading",
  "file_type": "string (optional) - Override language detection",
  "add_line_numbers": "boolean (default: false) - Add line numbers"
}
```

### convert_text
```json
{
  "content": "string (required) - Text content",
  "file_type": "string (optional) - Programming language",
  "title": "string (optional) - Markdown document title",
  "add_line_numbers": "boolean (default: false) - Add line numbers"
}
```

### convert_from_source (NEW)
```json
{
  "source": "string (required) - File path, URL, or '-' for stdin",
  "file_type": "string (optional) - Override language detection",
  "title": "string (optional) - Markdown document title",
  "add_line_numbers": "boolean (default: false) - Add line numbers"
}
```

### list_directory_files (NEW)
```json
{
  "directory": "string (required) - Directory path",
  "recursive": "boolean (default: true) - Scan subdirectories"
}
```

## Usage Examples

### Convert Python file with line numbers
```bash
echo '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"script.py","add_line_numbers":true}}}' | ./to_markdown_mcp
```

### Convert code from URL
```bash
echo '{"jsonrpc":"2.0","id":"2","method":"tools/call","params":{"name":"convert_from_source","arguments":{"source":"https://raw.githubusercontent.com/user/repo/main/src/main.rs"}}}' | ./to_markdown_mcp
```

### List all code files in project
```bash
echo '{"jsonrpc":"2.0","id":"3","method":"tools/call","params":{"name":"list_directory_files","arguments":{"directory":".","recursive":true}}}' | ./to_markdown_mcp
```

### Convert Makefile (no extension)
```bash
echo '{"jsonrpc":"2.0","id":"4","method":"tools/call","params":{"name":"convert_from_source","arguments":{"source":"Makefile"}}}' | ./to_markdown_mcp
```

### Convert with explicit language override
```bash
echo '{"jsonrpc":"2.0","id":"5","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"script","file_type":"python"}}}' | ./to_markdown_mcp
```

## Technical Details

### New Dependencies
- `reqwest` - Async HTTP client for fetching from URLs
- `url` - URL parsing and validation

### New Modules
- `sources.rs` - Handles different file sources (file paths, URLs, stdin, directories)
- Enhanced `converter.rs` - Line number support
- Expanded `file_type.rs` - 60+ language detection

### Cross-Platform
All new features are cross-platform compatible:
- Windows, Linux, macOS
- No platform-specific code
- Same binary works everywhere

### Performance
- Minimal overhead for line number addition
- Efficient directory scanning with early filtering
- Async HTTP requests for URL fetching

## Testing

Run the test suite to verify all enhancements:
```bash
cargo test
```

All 13+ tests pass, covering:
- File type detection (60+ languages)
- Source type parsing (file, URL, stdin)
- Code file identification
- Directory exclusion logic
- Conversion with line numbers
- Title and language handling
