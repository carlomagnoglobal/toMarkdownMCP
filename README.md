# toMarkdownMCP

A Model Context Protocol (MCP) server written in Rust that converts code files, HTML documents, and text content to Markdown format. Cross-platform compatible with Windows, Linux, and macOS.

Supports converting from multiple sources including local files, HTTP/HTTPS URLs, and stdin. Handles 60+ programming languages, HTML/HTM/MHTML web formats, office/document formats (PDF, DOCX, XLSX, PPTX, …), email, ebooks, feeds, and markup — and exposes **62 tools** in total, including Chromium-based web page capture, full Obsidian vault support (wikilinks, backlinks, canvas, dataview, templates), an AI/RAG toolkit, and optional Claude-backed generation. The same binary doubles as a terminal Markdown viewer (`to_markdown_mcp tui <vault>`).

## Features

### Code & Web Content Conversion
- **Convert code files** to Markdown from multiple sources:
  - Local file paths
  - HTTP/HTTPS URLs
  - stdin
  - Directory scanning with automatic detection
- **Convert HTML documents** to clean Markdown:
  - Standard HTML files (*.html)
  - Legacy HTM files (*.htm)
  - MHTML archives (*.mhtml) - single-file web archives
- **Auto-detect 60+ programming languages** from file extensions and filenames
- **Intelligent HTML parsing** - preserves structure, headings, lists, links, code blocks

### Formatting & Output
- **Line numbers** support for better code readability
- **Code syntax highlighting** with proper Markdown code blocks
- **Explicit language specification** for extension-less files
- **Title extraction** from HTML documents and filenames
- **Clean output** - removes excess whitespace and artifacts

### Platform & Protocol
- **Cross-platform** support (Windows, Linux, macOS)
- **JSON-RPC 2.0** MCP protocol implementation
- **Multiple file sources** - files, URLs, stdin, directories
- **Zero external MCP dependencies** - pure Rust implementation

## Supported File Types (60+ Languages + Web Formats)

### Web & Document Formats
- **HTML Documents**: `*.html`, `*.htm` - Standard HTML with full conversion to Markdown
- **MHTML Archives**: `*.mhtml` - MIME HTML archives (single-file web archives from browsers)
- **Webarchive**: `*.webarchive` - Safari/Apple webarchive format (macOS/iOS web archives)
- **HTML to Markdown** - Converts HTML structure (headings, lists, links, code blocks, etc.)

### Office & Document Formats
- **Documents**: `*.pdf`, `*.docx`, `*.doc`, `*.rtf`, `*.odt`
- **Spreadsheets**: `*.xlsx`, `*.xls`, `*.ods`, `*.csv` → Markdown tables (one section per sheet)
- **Presentations**: `*.pptx`, `*.odp` → per-slide headings and bullets
- **Email**: `*.eml` → headers as YAML frontmatter + converted body
- **Ebooks**: `*.epub`, `*.mobi` → chapters converted to Markdown
- **Feeds**: `*.rss`, `*.atom` → per-item headings, links, dates, content
- **Markup**: `*.wiki`/`*.mediawiki`, `*.rst`, `*.adoc`, `*.org`, `*.tex`, `*.textile` → real Markdown

See `DOCUMENT_CONVERSION.md` and `MARKUP_CONVERSION.md` for details and per-format behavior.

### AI / RAG / Knowledge Tools
Turn any converted document into machine-consumable retrieval units (all support
`output_format: "json"`):
- **RAG**: `chunk_markdown`, `extract_chunks_for_rag`, `get_document_outline`, `search_content`,
  `get_text_statistics`, `get_corpus_statistics`
- **Second brain**: `extract_tags`, `extract_keywords`, `find_related_notes`, `summarize_document`,
  `extract_qa_pairs`, `extract_entities`, `build_knowledge_index`
- **Retrieval & budgeting**: `retrieve_context` (assemble context for a query), `count_tokens`
- **Dedup & intelligence**: `find_duplicates`, `cluster_documents`, `analyze_readability`,
  `detect_natural_language`, `classify_document`
- **Claude-backed (optional, needs `ANTHROPIC_API_KEY`)**: `ai_summarize`, `ai_ask`, `ai_tag`,
  `ai_translate`, `ai_classify` — degrade gracefully to a setup note when no key is set

See `RAG_TOOLS.md`, `SECOND_BRAIN_TOOLS.md`, and `AI_TOOLS.md`.

### Programming Languages (60+)
The server auto-detects and properly formats code for:

- **Web**: CSS, SCSS, Sass, Less, JavaScript, JSX, TypeScript, TSX, Vue, Svelte, Astro
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

The server exposes **62 tools** — format conversion, browser-based web capture (Chromium, with
human-in-the-loop support for CAPTCHAs/logins — see [BROWSER_TOOLS.md](docs/tools/BROWSER_TOOLS.md)),
Obsidian vault support (wikilink/backlink graph, tasks, canvas, dataview, templates — see
[OBSIDIAN_TOOLS.md](docs/tools/OBSIDIAN_TOOLS.md)), file/vault operations, an AI/RAG toolkit, and optional
Claude-backed generation. Call `get_tool_help` (no arguments) for the full list, or with a
`tool_name` for detailed help on one tool. A selection is documented below.

Run `to_markdown_mcp tui [PATH]` for an interactive terminal Markdown viewer over a vault or
file: file tree, styled rendering, `[[wikilink]]` following, and filename search.

Start the server with `--base-dir /path/to/vault` (repeatable for multiple vaults) to set
default directories once in your MCP client config: relative paths then resolve against
them and `vault_path` can be omitted in tool calls — see [USAGE.md](docs/guides/USAGE.md).

## Documentation

**Guides**: [GETTING_STARTED.md](docs/guides/GETTING_STARTED.md) · [QUICK_START.md](docs/guides/QUICK_START.md) ·
[INSTALL.md](docs/guides/INSTALL.md) · [USAGE.md](docs/guides/USAGE.md) · [DEPLOYMENT.md](docs/deployment/DEPLOYMENT.md) ·
[RELEASE.md](docs/deployment/RELEASE.md) · [CHANGELOG.md](CHANGELOG.md)

**Feature docs**: [BROWSER_TOOLS.md](docs/tools/BROWSER_TOOLS.md) · [OBSIDIAN_TOOLS.md](docs/tools/OBSIDIAN_TOOLS.md) ·
[RAG_TOOLS.md](docs/tools/RAG_TOOLS.md) · [SECOND_BRAIN_TOOLS.md](docs/tools/SECOND_BRAIN_TOOLS.md) ·
[AI_TOOLS.md](docs/tools/AI_TOOLS.md) · [SRE_TOOLS.md](docs/tools/SRE_TOOLS.md) ·
[DOCUMENT_CONVERSION.md](docs/features/DOCUMENT_CONVERSION.md) · [MARKUP_CONVERSION.md](docs/features/MARKUP_CONVERSION.md)

**Publishing**: [DOCKER_HUB_QUICK_START.md](docs/deployment/DOCKER_HUB_QUICK_START.md) ·
[MCP_REGISTRIES_GUIDE.md](docs/deployment/MCP_REGISTRIES_GUIDE.md) ·
[PUBLISH_TO_REGISTRIES.md](docs/deployment/PUBLISH_TO_REGISTRIES.md) ·
validation: [MCP_TEST_RESULTS.md](docs/planning/MCP_TEST_RESULTS.md)

#### 1. `convert_file`
Converts a file to Markdown format. For HTML files, supports optional metadata extraction, CSS hints, TOC generation, and image handling.

**Parameters:**
- `file_path` (string, required): Path to the file to convert
- `include_filename` (boolean, optional): Include filename as heading (default: true)
- `file_type` (string, optional): Explicitly specify language (overrides detection)
- `add_line_numbers` (boolean, optional): Add line numbers to code block (default: false)
- `extract_metadata` (boolean, optional): Extract metadata from HTML as YAML frontmatter (default: false)
- `preserve_css_hints` (boolean, optional): Preserve CSS styling hints as comments (default: false)
- `convert_tables` (boolean, optional): Convert HTML tables to Markdown pipe tables (default: false)
- `extract_forms` (boolean, optional): Extract HTML forms as Markdown tables (default: false)
- `preserve_comments` (boolean, optional): Preserve HTML comments in output (default: false)
- `extract_links` (boolean, optional): Extract and summarize all links in document (default: false)
- `analyze_headings` (boolean, optional): Analyze heading structure and hierarchy (default: false)
- `extract_images` (boolean, optional): Extract and process images (default: false)
- `image_format` (string, optional): Image output format: "link" or "skip" (default: "link")
- `generate_toc` (boolean, optional): Generate table of contents from headings (default: false)
- `toc_max_level` (integer, optional): Maximum heading level in TOC, 1-6 (default: 3)

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
Converts code or HTML from various sources (file, URL, stdin) to Markdown format. For HTML, supports optional metadata extraction, CSS hints, TOC generation, and image handling.

**Parameters:**
- `source` (string, required): File path, HTTP/HTTPS URL, or `-` for stdin
- `file_type` (string, optional): Explicitly specify language (overrides detection)
- `title` (string, optional): Title for the Markdown document
- `add_line_numbers` (boolean, optional): Add line numbers to code block (default: false)
- `extract_metadata` (boolean, optional): Extract metadata from HTML as YAML frontmatter (default: false)
- `preserve_css_hints` (boolean, optional): Preserve CSS styling hints as comments (default: false)
- `convert_tables` (boolean, optional): Convert HTML tables to Markdown pipe tables (default: false)
- `extract_forms` (boolean, optional): Extract HTML forms as Markdown tables (default: false)
- `preserve_comments` (boolean, optional): Preserve HTML comments in output (default: false)
- `extract_links` (boolean, optional): Extract and summarize all links in document (default: false)
- `analyze_headings` (boolean, optional): Analyze heading structure and hierarchy (default: false)
- `extract_images` (boolean, optional): Extract and process images (default: false)
- `image_format` (string, optional): Image output format: "link" or "skip" (default: "link")
- `generate_toc` (boolean, optional): Generate table of contents from headings (default: false)
- `toc_max_level` (integer, optional): Maximum heading level in TOC, 1-6 (default: 3)

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

## HTML to Markdown Conversion

The server can convert HTML documents to clean, readable Markdown format. This includes standard HTML files, legacy HTM files, and MHTML archives.

### Supported HTML Formats

#### *.html - Standard HTML Files
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html"
  }
}
```

#### *.htm - Legacy HTML Files
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "old_page.htm"
  }
}
```

#### *.mhtml - MIME HTML Archives
Single-file web archives created by IE, Edge, and other browsers:
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "archived_page.mhtml"
  }
}
```

### What Gets Converted

✓ **Structure** - Headings (h1-h6), paragraphs, line breaks
✓ **Lists** - Unordered (ul) and ordered (ol) lists
✓ **Formatting** - Bold, italic, strikethrough text
✓ **Code** - Inline code and code blocks (pre/code)
✓ **Links** - Hyperlinks with URL preservation
✓ **Blockquotes** - Quoted text blocks
✓ **Titles** - Document titles from `<title>` tags
✓ **Nesting** - Proper hierarchy preservation

### Example Conversion

**Input HTML:**
```html
<h1>Getting Started with Rust</h1>
<p>Rust is a systems programming language focused on safety.</p>
<h2>Key Features</h2>
<ul>
  <li><strong>Memory Safety</strong> - No data races</li>
  <li><strong>Performance</strong> - Zero-cost abstractions</li>
  <li><strong>Concurrency</strong> - Safe parallel programming</li>
</ul>
<p>Learn more at <a href="https://www.rust-lang.org/">rust-lang.org</a></p>
```

**Output Markdown:**
```markdown
# Getting Started with Rust

Getting Started with Rust
==========

Rust is a systems programming language focused on safety.

Key Features
----------

* **Memory Safety** - No data races
* **Performance** - Zero-cost abstractions
* **Concurrency** - Safe parallel programming

Learn more at [rust-lang.org](https://www.rust-lang.org/)
```

### Advanced HTML Conversion Options

#### Metadata Extraction
Extract document metadata (title, author, description) as YAML frontmatter:
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "extract_metadata": true
  }
}
```

#### CSS Styling Hints
Preserve CSS styling information as HTML comments:
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "styled_page.html",
    "preserve_css_hints": true
  }
}
```

#### Table Conversion
Convert HTML tables to Markdown pipe tables:
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "data_table.html",
    "convert_tables": true
  }
}
```

**Features:**
- Converts `<table>` elements to Markdown pipe syntax
- Handles simple and structured tables (thead/tbody)
- Preserves cell content and formatting
- Supports multiple tables in one document

#### Image Extraction
Control how images are handled during conversion:
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page_with_images.html",
    "extract_images": true,
    "image_format": "link"
  }
}
```

**Image format options:**
- `"link"` (default) - Keep external image URLs in Markdown syntax
- `"skip"` - Remove images, keep alt text only
- `"embed"` (planned) - Convert images to base64 for embedding

#### Table of Contents Generation
Auto-generate a linked table of contents from headings:
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "long_article.html",
    "generate_toc": true,
    "toc_max_level": 3
  }
}
```

#### Combined Features
All HTML features work together seamlessly:
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "article.html",
    "extract_metadata": true,
    "convert_tables": true,
    "extract_images": true,
    "image_format": "link",
    "preserve_css_hints": true,
    "generate_toc": true,
    "toc_max_level": 3
  }
}
```

For detailed documentation on each feature:
- [METADATA_EXTRACTION.md](docs/features/METADATA_EXTRACTION.md) - Extract document metadata
- [CSS_STYLING_HINTS.md](docs/features/CSS_STYLING_HINTS.md) - Preserve CSS information
- [IMAGE_EXTRACTION.md](docs/features/IMAGE_EXTRACTION.md) - Control image handling
- [TABLE_CONVERSION.md](docs/features/TABLE_CONVERSION.md) - Convert HTML tables to Markdown
- [FORM_EXTRACTION.md](docs/features/FORM_EXTRACTION.md) - Extract and convert HTML forms
- [HEADING_ANALYSIS.md](docs/features/HEADING_ANALYSIS.md) - Analyze heading structure and hierarchy
- [LINK_EXTRACTION.md](docs/features/LINK_EXTRACTION.md) - Extract and summarize links
- [COMMENT_PRESERVATION.md](docs/features/COMMENT_PRESERVATION.md) - Preserve HTML comments
- [CODE_LANGUAGE_DETECTION.md](docs/features/CODE_LANGUAGE_DETECTION.md) - Auto-detect code block languages
- [TOC_GENERATION.md](docs/features/TOC_GENERATION.md) - Generate table of contents
- [WEBARCHIVE_SUPPORT.md](docs/features/WEBARCHIVE_SUPPORT.md) - Safari webarchive support

For complete HTML conversion documentation, see [HTML_SUPPORT.md](docs/features/HTML_SUPPORT.md).

## HTML Table Conversion

Convert HTML tables to clean Markdown pipe format:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "data.html",
    "convert_tables": true
  }
}
```

**Features:**
- Converts all HTML `<table>` elements to Markdown syntax
- Handles both simple and structured tables (thead/tbody)
- Preserves cell content with proper text extraction
- Supports multiple tables in one document
- Works with formatted content (bold, italic, etc.)
- Escapes special characters appropriately

**Example Input:**
```html
<table>
  <tr>
    <th>Name</th>
    <th>Age</th>
  </tr>
  <tr>
    <td>Alice</td>
    <td>30</td>
  </tr>
</table>
```

**Example Output:**
```markdown
| Name | Age |
| :--- | :--- |
| Alice | 30 |
```

See [TABLE_CONVERSION.md](docs/features/TABLE_CONVERSION.md) for complete documentation.

## Form Extraction

Extract HTML forms and convert them to readable Markdown tables:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "contact.html",
    "extract_forms": true
  }
}
```

**Features:**
- Extracts all `<form>` elements from HTML
- Converts to Markdown tables with field information
- Captures field names, types, labels, and requirements
- Supports input types: text, email, password, number, checkbox, radio, select, textarea, etc.
- Preserves form attributes (method, action, enctype)
- Useful for documentation and content migration

**Example Input:**
```html
<form method="POST" action="/contact">
  <label for="name">Name:</label>
  <input type="text" id="name" name="name" required>
  
  <label for="email">Email:</label>
  <input type="email" id="email" name="email" required>
  
  <button type="submit">Send</button>
</form>
```

**Example Output:**
```markdown
### Form

**Action:** /contact

**Method:** POST

| Field | Type | Required | Details |
|-------|------|----------|---------|
| name | text | ✓ | Label: *Name* |
| email | email | ✓ | Label: *Email* |
| | submit | | Label: Send |
```

See [FORM_EXTRACTION.md](docs/features/FORM_EXTRACTION.md) for complete documentation.

## Comment Preservation

Extract and preserve HTML comments during conversion:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "preserve_comments": true
  }
}
```

**Features:**
- Extracts all `<!-- -->` comments from HTML
- Detects comment types (regular, directives, conditional)
- Generates summary of comments found
- Preserves IE conditional comments
- Captures developer notes and TODOs
- Useful for documentation and code review

**Example Input:**
```html
<!-- Main page -->
<h1>Welcome</h1>
<!-- TODO: Add footer section -->
<!--[if IE]><p>IE Only</p><![endif]-->
```

**Example Output:**
```markdown
## Comments Found

**Total:** 3 comments

**Conditional:** 1 | **Regular:** 2

### Conditional Comments (IE)

1. `[if IE]><p>IE Only</p><![endif]`

### Regular Comments

1. `Main page`
2. `TODO: Add footer section`

# Welcome
```

See [COMMENT_PRESERVATION.md](docs/features/COMMENT_PRESERVATION.md) for complete documentation.

## Link Extraction

Extract and categorize all hyperlinks in a document:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "extract_links": true
  }
}
```

**Features:**
- Extracts all `<a>` links from HTML
- Categorizes as external, internal, or broken
- Captures link text, URL, title, target, and rel attributes
- Generates summary with statistics and categorization
- Identifies invalid links (empty href, anchors, javascript:)

**Example Input:**
```html
<a href="https://example.com">External Link</a>
<a href="/page">Internal Link</a>
<a href="">Broken Link</a>
<a href="#section">Anchor</a>
```

**Example Output:**
```markdown
## Links Found

**Total:** 3 links

**External:** 1 | **Internal:** 1 | **Broken:** 1

### External Links

1. [External Link](https://example.com)

### Internal Links

1. [Internal Link](/page)

### Broken/Invalid Links

1. `Broken Link`
```

See [LINK_EXTRACTION.md](docs/features/LINK_EXTRACTION.md) for complete documentation.

## Heading Structure Analysis

Analyze document heading hierarchy and validate structure:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "analyze_headings": true
  }
}
```

**Features:**
- Extracts all h1-h6 headings in document order
- Validates proper heading hierarchy
- Detects issues (jumps, multiple h1s, broken nesting)
- Generates statistics on heading distribution
- Creates visual tree representation
- Important for accessibility and SEO

**Example Output:**
```markdown
## Heading Structure Analysis

**Total Headings:** 4

### Heading Levels Distribution

| Level | Count |
|-------|-------|
| H1 | 1 |
| H2 | 2 |
| H3 | 1 |

**Hierarchy Depth:** 1 - 3

### ✓ No Hierarchy Issues

## Document Heading Structure

```
├─ H1: Main Title
  ├─ H2: Section 1
    ├─ H3: Subsection
  ├─ H2: Section 2
```

See [HEADING_ANALYSIS.md](docs/features/HEADING_ANALYSIS.md) for complete documentation.

## Webarchive Support

Convert Safari web archives (`.webarchive`) to Markdown:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "webpage.webarchive",
    "extract_metadata": true,
    "generate_toc": true
  }
}
```

Webarchive files from Safari, Mail, and other Apple applications are fully supported with:
- Automatic HTML extraction from the plist container
- Support for all HTML conversion features (metadata, images, CSS hints, TOC, tables)
- UTF-8 and binary data handling
- Cross-platform compatibility

See [WEBARCHIVE_SUPPORT.md](docs/features/WEBARCHIVE_SUPPORT.md) for complete documentation.

## Testing

Run the test suite:

```bash
cargo test
```

### Example Test Files

Pre-built examples are in the `examples/` directory:

**Code Files:**
- `examples/test.py` - Python example
- `examples/test.rs` - Rust example
- `examples/test.js` - JavaScript example
- `examples/test.txt` - Text file

**HTML/Web Examples:**
- `examples/sample.html` - Standard HTML document
- `examples/sample.htm` - Legacy HTM format
- `examples/sample.mhtml` - MHTML archive
- `examples/sample.webarchive` - Safari webarchive format (macOS/iOS)
- `examples/sample_with_metadata.html` - HTML with rich metadata for testing metadata extraction
- `examples/styled_page.html` - HTML with inline CSS styling for CSS hints testing
- `examples/toc_demo.html` - Multi-level heading structure for TOC generation testing
- `examples/images_demo.html` - Multiple image examples for testing image extraction
- `examples/table_demo.html` - Various table types for testing table conversion
- `examples/code_blocks_demo.html` - Code blocks in multiple languages for language detection testing
- `examples/form_demo.html` - Various HTML forms for testing form extraction
- `examples/comment_demo.html` - HTML comments of various types for testing comment preservation
- `examples/link_demo.html` - Various link types for testing link extraction

Or create your own test files:

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

# Create a test HTML file
cat > test.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Test Page</title>
</head>
<body>
    <h1>Hello World</h1>
    <p>This is a test HTML page.</p>
</body>
</html>
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

### Core Modules
- **main.rs**: MCP server implementation and request handling
- **converter.rs**: Markdown conversion logic for code files
- **file_type.rs**: Programming language detection (60+ languages)
- **error.rs**: Custom error types

### Additional Modules
- **html_converter.rs**: HTML/HTM/MHTML parsing and conversion to Markdown
- **sources.rs**: Multi-source file handling (local files, URLs, stdin, directories)

### Module Responsibilities
- **HTML Conversion**: Parses HTML using CSS selectors, converts to clean Markdown
- **Source Handling**: Detects file type (file, URL, stdin), fetches content
- **File Discovery**: Directory scanning with auto-exclusion of common non-code directories
- **Language Detection**: Extension-based and filename-based language identification

## Documentation

- **README.md** - This file, getting started guide
- **HTML_SUPPORT.md** - Detailed HTML conversion documentation
- **QUICK_START.md** - Quick reference guide
- **ENHANCEMENTS.md** - Feature enhancements and advanced usage

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
