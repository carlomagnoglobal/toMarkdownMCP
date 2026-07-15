#![recursion_limit = "512"]
// Several modules intentionally expose ready-to-use helper APIs (alternate
// output formats, extraction utilities) that aren't all wired into a tool yet.
#![allow(dead_code)]
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

mod converter;
mod embeddings;
mod file_type;
mod error;
mod sources;
mod html_converter;
mod toc_generator;
mod image_extractor;
mod webarchive_parser;
mod table_converter;
mod code_language_detector;
mod form_extractor;
mod comment_extractor;
mod link_extractor;
mod heading_analyzer;
mod blockquote_extractor;
mod definition_list_converter;
mod document_converter;
mod office_converter;
mod feed_email_converter;
mod markup_converter;
mod rag;
mod knowledge;
mod retrieval;
mod similarity;
mod doc_intel;
mod llm;
mod browser;
mod obsidian;
mod textmetrics;
mod tui;

use converter::convert_to_markdown_with_options;
use file_type::{detect_language, detect_language_from_filename};
use error::ConversionError;
use sources::{SourceType, fetch_from_source, list_files_in_directory};
use html_converter::{html_to_markdown_with_options, extract_html_from_mhtml};
use toc_generator::{generate_toc, format_toc};
use image_extractor::ImageFormat;

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    // Number or string per JSON-RPC; absent for notifications.
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    // JSON-RPC: a response carries exactly one of result/error — emitting a
    // null for the other key fails strict clients (Claude Desktop's schema).
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// Base/vault directories from `--base-dir` flags. First entry is the
/// primary default. Empty vec = feature off (fully backward compatible).
static BASE_DIRS: once_cell::sync::OnceCell<Vec<std::path::PathBuf>> = once_cell::sync::OnceCell::new();

fn base_dirs() -> &'static [std::path::PathBuf] {
    BASE_DIRS.get().map(|v| v.as_slice()).unwrap_or(&[])
}

fn expand_tilde(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::Path::new(&home).join(rest).to_string_lossy().into_owned();
        }
    }
    p.to_string()
}

/// Resolve `--base-dir` values (tilde-expanded, canonicalized) and store them
/// in the process-global BASE_DIRS. Empty input leaves the feature off.
fn set_base_dirs(values: &[String]) {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    for part in values.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let p = std::path::PathBuf::from(expand_tilde(part));
        if !p.is_dir() {
            eprintln!("warning: --base-dir '{}' is not an existing directory (kept anyway)", part);
        }
        dirs.push(p.canonicalize().unwrap_or(p));
    }
    if !dirs.is_empty() {
        let _ = BASE_DIRS.set(dirs);
    }
}

#[derive(clap::Parser)]
#[command(
    name = "to_markdown_mcp",
    version,
    about = "Markdown conversion MCP server, vault viewer, and CLI",
    long_about = "Run with no subcommand to start the MCP server (JSON-RPC over stdio).\n\
                  Subcommands expose the converters directly from the terminal."
)]
struct Cli {
    /// Default vault/base directory. Repeatable (or comma-separated) for
    /// multiple vaults; the first is the primary default. Relative tool paths
    /// resolve against these directories, and vault_path/directory parameters
    /// may be omitted in tool calls.
    #[arg(long = "base-dir", global = true, value_delimiter = ',', value_name = "DIR")]
    base_dir: Vec<String>,

    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(clap::Subcommand)]
enum CliCommand {
    /// Open the TUI Markdown viewer on a vault dir or file (default: .)
    Tui {
        path: Option<String>,
    },
    /// Convert a file, URL, or stdin ('-') to Markdown
    Convert {
        /// File path, URL (http://...), or '-' for stdin
        source: String,
        /// Write output to this file instead of stdout
        #[arg(short, long, value_name = "FILE")]
        output: Option<std::path::PathBuf>,
        /// Override language auto-detection (e.g. python, makefile)
        #[arg(long = "type", value_name = "LANG")]
        file_type: Option<String>,
        /// Add line numbers to code blocks
        #[arg(long)]
        line_numbers: bool,
        /// Title for the Markdown document
        #[arg(long)]
        title: Option<String>,
    },
    /// Convert multiple files to Markdown in one combined document (up to 10)
    Batch {
        /// Files to convert
        #[arg(required = true)]
        files: Vec<String>,
        /// Write combined output to this file instead of stdout
        #[arg(short, long, value_name = "FILE")]
        output: Option<std::path::PathBuf>,
    },
    /// Search inside converted document content across a directory
    Search {
        /// Search terms
        query: String,
        /// Directory to search (recursive)
        #[arg(long, default_value = ".", value_name = "DIR")]
        dir: String,
        /// Maximum number of results
        #[arg(long, default_value_t = 10)]
        max_results: u32,
    },
    /// Print the MCP tool catalog, or detailed help for one tool
    Tools {
        tool_name: Option<String>,
    },
}

/// Handlers return `Box<dyn Error>` (not Send+Sync); repackage for anyhow.
fn cli_err(e: Box<dyn std::error::Error>) -> anyhow::Error {
    anyhow::anyhow!("{}", e)
}

/// Print to stdout or write to a file when `-o` was given.
fn emit(output: Option<&std::path::Path>, content: &str) -> Result<()> {
    match output {
        Some(path) => {
            std::fs::write(path, content)?;
            eprintln!("Wrote {}", path.display());
        }
        None => println!("{}", content),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // CLI dispatch: no subcommand = MCP stdio server (unchanged default).
    let cli = <Cli as clap::Parser>::parse();
    set_base_dirs(&cli.base_dir);
    match cli.command {
        None => {}
        Some(CliCommand::Tui { path }) => {
            let default = base_dirs()
                .first()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| ".".to_string());
            let path = path.unwrap_or(default);
            return tui::run(Path::new(&path));
        }
        Some(CliCommand::Convert { source, output, file_type, line_numbers, title }) => {
            let mut args = json!({"source": source, "add_line_numbers": line_numbers});
            if let Some(t) = file_type {
                args["file_type"] = json!(t);
            }
            if let Some(t) = title {
                args["title"] = json!(t);
            }
            let md = handle_convert_from_source(&args).await.map_err(cli_err)?;
            return emit(output.as_deref(), &md);
        }
        Some(CliCommand::Batch { files, output }) => {
            let args = json!({"file_paths": files});
            let md = handle_batch_convert_files(&args).map_err(cli_err)?;
            return emit(output.as_deref(), &md);
        }
        Some(CliCommand::Search { query, dir, max_results }) => {
            let args = json!({"query": query, "directory": dir, "max_results": max_results});
            let result = handle_search_content(&args).map_err(cli_err)?;
            println!("{}", result);
            return Ok(());
        }
        Some(CliCommand::Tools { tool_name }) => {
            let args = match tool_name {
                Some(name) => json!({"tool_name": name}),
                None => json!({}),
            };
            let help = handle_get_tool_help(&args).map_err(cli_err)?;
            println!("{}", help);
            return Ok(());
        }
    }

    // Log to stderr: stdout is reserved for the JSON-RPC stream.
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(std::io::stderr)
        .init();

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = stdin.lock();

    // Process incoming messages
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => {
                // JSON-RPC notifications (no id) must not receive a response.
                if request.id.is_none() {
                    continue;
                }
                let response = handle_request(&request).await;
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
            }
            Err(e) => {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": "Parse error",
                        "data": e.to_string()
                    }
                });
                writeln!(stdout, "{}", serde_json::to_string(&error_response)?)?;
                stdout.flush()?;
            }
        }
    }

    Ok(())
}

fn request_id(request: &JsonRpcRequest) -> Value {
    request.id.clone().unwrap_or(Value::Null)
}

async fn handle_request(request: &JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
            // Echo the client's protocol version when given (MCP handshake).
            let version = request
                .params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("2024-11-05");
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id(request),
                result: Some(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
                    "serverInfo": {
                        "name": "toMarkdownMCP",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                })),
                error: None,
            }
        }
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request_id(request),
            result: Some(json!({})),
            error: None,
        },
        "tools/list" => handle_list_tools(&request_id(request)),
        "tools/call" => handle_call_tool(request).await,
        "resources/list" => handle_list_resources(&request_id(request)),
        "resources/read" => handle_read_resource(request),
        "prompts/list" => handle_list_prompts(&request_id(request)),
        "prompts/get" => handle_get_prompt(request),
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request_id(request),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        },
    }
}

/// The full tools/list array — single source of truth for tool names,
/// descriptions, and input schemas (also drives get_tool_help fallback).
fn tool_definitions() -> Value {
    json!([
        {
            "name": "convert_file",
            "description": "Convert a text, code, HTML, or document file to Markdown format. Supports HTML/HTM/MHTML/webarchive; documents PDF, DOCX, DOC, RTF, ODT; spreadsheets XLSX, XLS, ODS, CSV; presentations PPTX, ODP; email EML; ebooks EPUB, MOBI; RSS/Atom feeds; and markup WIKI, RST, ADOC, ORG, TEX, TEXTILE.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to convert"
                    },
                    "max_bytes": {
                        "type": "integer",
                        "description": "Size limit in bytes for structured formats (default 10485760 = 10MB); larger plain-text files stream instead"
                    },
                    "include_filename": {
                        "type": "boolean",
                        "description": "Include filename as Markdown heading (default: true)",
                        "default": true
                    },
                    "file_type": {
                        "type": "string",
                        "description": "Optional: explicitly specify language (overrides detection)"
                    },
                    "add_line_numbers": {
                        "type": "boolean",
                        "description": "Add line numbers to code block (default: false)",
                        "default": false
                    },
                    "extract_metadata": {
                        "type": "boolean",
                        "description": "Extract metadata from HTML files as YAML frontmatter (default: false)",
                        "default": false
                    },
                    "preserve_css_hints": {
                        "type": "boolean",
                        "description": "Preserve CSS styling hints as HTML comments in output (default: false)",
                        "default": false
                    },
                    "generate_toc": {
                        "type": "boolean",
                        "description": "Generate table of contents from headings (default: false)",
                        "default": false
                    },
                    "toc_max_level": {
                        "type": "integer",
                        "description": "Maximum heading level to include in TOC (1-6, default: 3)",
                        "default": 3
                    },
                    "extract_images": {
                        "type": "boolean",
                        "description": "Extract and process images from HTML (default: false)",
                        "default": false
                    },
                    "image_format": {
                        "type": "string",
                        "description": "Image output format: 'link' (external URLs), 'skip' (remove), or 'embed' (base64, default: 'link')",
                        "default": "link"
                    },
                    "convert_tables": {
                        "type": "boolean",
                        "description": "Convert HTML tables to Markdown pipe tables (default: false)",
                        "default": false
                    },
                    "extract_forms": {
                        "type": "boolean",
                        "description": "Extract HTML forms and convert to Markdown tables (default: false)",
                        "default": false
                    },
                    "preserve_comments": {
                        "type": "boolean",
                        "description": "Preserve HTML comments in output (default: false)",
                        "default": false
                    },
                    "extract_links": {
                        "type": "boolean",
                        "description": "Extract and summarize all links in document (default: false)",
                        "default": false
                    },
                    "analyze_headings": {
                        "type": "boolean",
                        "description": "Analyze heading structure and hierarchy (default: false)",
                        "default": false
                    },
                    "extract_definition_lists": {
                        "type": "boolean",
                        "description": "Extract and convert HTML definition lists (default: false)",
                        "default": false
                    },
                    "extract_blockquotes": {
                        "type": "boolean",
                        "description": "Extract and convert HTML blockquotes (default: false)",
                        "default": false
                    }
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "convert_text",
            "description": "Convert plain text content to Markdown format",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The text content to convert"
                    },
                    "file_type": {
                        "type": "string",
                        "description": "Optional: specify code language (e.g., 'rust', 'python', 'javascript')"
                    },
                    "title": {
                        "type": "string",
                        "description": "Optional: title for the markdown document"
                    },
                    "add_line_numbers": {
                        "type": "boolean",
                        "description": "Add line numbers to code block (default: false)",
                        "default": false
                    }
                },
                "required": ["content"]
            }
        },
        {
            "name": "convert_from_source",
            "description": "Convert code or HTML from various sources (file, URL, stdin) to Markdown",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source: file path, URL (http://...), or '-' for stdin"
                    },
                    "file_type": {
                        "type": "string",
                        "description": "Optional: specify code language (auto-detected from file extension if omitted)"
                    },
                    "title": {
                        "type": "string",
                        "description": "Optional: title for the markdown document"
                    },
                    "add_line_numbers": {
                        "type": "boolean",
                        "description": "Add line numbers to code block (default: false)",
                        "default": false
                    },
                    "extract_metadata": {
                        "type": "boolean",
                        "description": "Extract metadata from HTML as YAML frontmatter (default: false)",
                        "default": false
                    },
                    "preserve_css_hints": {
                        "type": "boolean",
                        "description": "Preserve CSS styling hints as HTML comments (default: false)",
                        "default": false
                    },
                    "generate_toc": {
                        "type": "boolean",
                        "description": "Generate table of contents from headings (default: false)",
                        "default": false
                    },
                    "toc_max_level": {
                        "type": "integer",
                        "description": "Maximum heading level to include in TOC (1-6, default: 3)",
                        "default": 3
                    },
                    "extract_images": {
                        "type": "boolean",
                        "description": "Extract and process images from HTML (default: false)",
                        "default": false
                    },
                    "image_format": {
                        "type": "string",
                        "description": "Image output format: 'link' (external URLs), 'skip' (remove), or 'embed' (base64, default: 'link')",
                        "default": "link"
                    },
                    "convert_tables": {
                        "type": "boolean",
                        "description": "Convert HTML tables to Markdown pipe tables (default: false)",
                        "default": false
                    },
                    "extract_forms": {
                        "type": "boolean",
                        "description": "Extract HTML forms and convert to Markdown tables (default: false)",
                        "default": false
                    },
                    "preserve_comments": {
                        "type": "boolean",
                        "description": "Preserve HTML comments in output (default: false)",
                        "default": false
                    },
                    "extract_links": {
                        "type": "boolean",
                        "description": "Extract and summarize all links in document (default: false)",
                        "default": false
                    },
                    "analyze_headings": {
                        "type": "boolean",
                        "description": "Analyze heading structure and hierarchy (default: false)",
                        "default": false
                    },
                    "extract_definition_lists": {
                        "type": "boolean",
                        "description": "Extract and convert HTML definition lists (default: false)",
                        "default": false
                    },
                    "extract_blockquotes": {
                        "type": "boolean",
                        "description": "Extract and convert HTML blockquotes (default: false)",
                        "default": false
                    }
                },
                "required": ["source"]
            }
        },
        {
            "name": "list_directory_files",
            "description": "List all code files in a directory (recursively)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {
                        "type": "string",
                        "description": "Directory path to scan for code files"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Recursively scan subdirectories (default: true)",
                        "default": true
                    }
                },
                "required": ["directory"]
            }
        },
        {
            "name": "get_file_summary",
            "description": "Get a lightweight summary of a file including metadata, headings, and preview",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to summarize"
                    },
                    "preview_length": {
                        "type": "integer",
                        "description": "Number of characters to include in preview (default: 300)",
                        "default": 300
                    }
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "batch_convert_files",
            "description": "Convert multiple files to Markdown in one call",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of file paths to convert (up to 10)"
                    },
                    "extract_metadata": {
                        "type": "boolean",
                        "description": "Extract metadata from HTML files (default: false)",
                        "default": false
                    },
                    "convert_tables": {
                        "type": "boolean",
                        "description": "Convert HTML tables to Markdown (default: false)",
                        "default": false
                    },
                    "extract_images": {
                        "type": "boolean",
                        "description": "Extract images from HTML (default: false)",
                        "default": false
                    },
                    "image_format": {
                        "type": "string",
                        "description": "Image format: 'link' (default), 'skip', or 'embed'",
                        "default": "link"
                    },
                    "extract_forms": {
                        "type": "boolean",
                        "description": "Extract HTML forms (default: false)",
                        "default": false
                    },
                    "extract_links": {
                        "type": "boolean",
                        "description": "Extract and summarize links (default: false)",
                        "default": false
                    }
                },
                "required": ["file_paths"]
            }
        },
        {
            "name": "search_files",
            "description": "Search for text across files and return matching snippets",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {
                        "type": "string",
                        "description": "Directory to search (recursively)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Text to search for (case-insensitive)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of files with matches (default: 5)",
                        "default": 5
                    },
                    "context_chars": {
                        "type": "integer",
                        "description": "Characters of context around match (default: 150)",
                        "default": 150
                    }
                },
                "required": ["directory", "query"]
            }
        },
        {
            "name": "get_recently_modified_files",
            "description": "List recently modified files in a directory",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {
                        "type": "string",
                        "description": "Directory to scan"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of files to return (default: 10)",
                        "default": 10
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Recursively scan subdirectories (default: true)",
                        "default": true
                    }
                },
                "required": ["directory"]
            }
        },
        {
            "name": "get_vault_statistics",
            "description": "Returns statistics about a directory: file counts by type, top-level folder list, and top 20 most frequent tags from Markdown files",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {
                        "type": "string",
                        "description": "Root directory to analyze"
                    }
                },
                "required": ["directory"]
            }
        },
        {
            "name": "extract_active_todos",
            "description": "Scans for uncompleted tasks (lines containing '- [ ]') and returns them grouped by file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {
                        "type": "string",
                        "description": "Root directory to scan (default: current directory)",
                        "default": "."
                    },
                    "folder_scope": {
                        "type": "string",
                        "description": "Optional subfolder path to restrict scan"
                    }
                },
                "required": []
            }
        },
        {
            "name": "safe_append_or_replace_section",
            "description": "Updates content under a specific heading in a Markdown file without touching the rest of the document",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the Markdown file"
                    },
                    "heading": {
                        "type": "string",
                        "description": "Heading name to target (without # symbols, e.g. 'Action Items')"
                    },
                    "content": {
                        "type": "string",
                        "description": "Markdown content to insert"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["append", "overwrite"],
                        "description": "'append' adds to bottom of section; 'overwrite' replaces everything under the heading",
                        "default": "append"
                    }
                },
                "required": ["path", "heading", "content"]
            }
        },
        {
            "name": "resolve_and_validate_links",
            "description": "Validates a list of note names or file paths against real files on disk",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "links": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Note names, filenames, or paths to validate"
                    },
                    "directory": {
                        "type": "string",
                        "description": "Root directory to search (default: current directory)",
                        "default": "."
                    }
                },
                "required": ["links"]
            }
        },
        {
            "name": "upsert_markdown_table",
            "description": "Inserts or overwrites a Markdown table under a specific heading. Creates file if it does not exist",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the Markdown file (will be created if absent)"
                    },
                    "heading": {
                        "type": "string",
                        "description": "Section heading under which the table should be placed"
                    },
                    "headers": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Column names for the table"
                    },
                    "rows": {
                        "type": "array",
                        "items": { "type": "array", "items": { "type": "string" } },
                        "description": "Array of rows, each an array of cell strings"
                    }
                },
                "required": ["path", "heading", "headers", "rows"]
            }
        },
        {
            "name": "read_file",
            "description": "Read raw file content as-is, without any conversion or processing",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "First line to return (default: 1)",
                        "default": 1
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Last line to return (omit for full file)"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "create_or_append_file",
            "description": "Create a new file with content, or append/overwrite an existing file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to create or modify"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write or append"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["create", "append", "overwrite"],
                        "description": "Mode: 'create' (fails if exists), 'append' (add to end), 'overwrite' (replace entirely)",
                        "default": "create"
                    }
                },
                "required": ["path", "content"]
            }
        },
        {
            "name": "get_tool_help",
            "description": "Get help on available tools. Specify a tool name for detailed help, or omit for a summary of all tools",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of a specific tool (omit for full list)"
                    }
                },
                "required": []
            }
        },
        {
            "name": "move_or_rename_file",
            "description": "Move or rename a file or directory to a new path",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "old_path": {
                        "type": "string",
                        "description": "Current file or directory path"
                    },
                    "new_path": {
                        "type": "string",
                        "description": "Destination path (including new filename)"
                    }
                },
                "required": ["old_path", "new_path"]
            }
        },
        {
            "name": "delete_file",
            "description": "Permanently delete a file or directory (irreversible)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to file or directory to delete"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Required true to delete non-empty directories (default: false)",
                        "default": false
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "batch_create_notes",
            "description": "Create multiple new files in one call",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "notes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "File path (parent directories auto-created)"
                                },
                                "content": {
                                    "type": "string",
                                    "description": "File content"
                                }
                            },
                            "required": ["path", "content"]
                        },
                        "description": "Array of {path, content} objects (up to 20)"
                    }
                },
                "required": ["notes"]
            }
        },
        {
            "name": "update_note_properties",
            "description": "Update YAML frontmatter properties in a Markdown file without changing the body",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the Markdown file"
                    },
                    "properties": {
                        "type": "object",
                        "description": "Key-value pairs to set or update (set value to null to remove)"
                    }
                },
                "required": ["path", "properties"]
            }
        },
        {
            "name": "find_note_by_alias_or_title",
            "description": "Fuzzy lookup to find a file by name, filename snippet, or frontmatter alias",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "search_term": {
                        "type": "string",
                        "description": "Conceptual name, filename snippet, or alias to search for"
                    },
                    "directory": {
                        "type": "string",
                        "description": "Root directory to search (default: current directory)",
                        "default": "."
                    }
                },
                "required": ["search_term"]
            }
        },
        {
            "name": "get_graph_relationships",
            "description": "Map wiki-link connections: outlinks (references this file makes) and backlinks (files that reference this file)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to analyze"
                    },
                    "directory": {
                        "type": "string",
                        "description": "Root directory to scan for backlinks (default: current directory)",
                        "default": "."
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "chunk_markdown",
            "description": "Split a Markdown/document file into heading-aware, token-bounded chunks for RAG/embeddings. Dual output: readable Markdown or structured JSON (output_format=json).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to chunk (any supported format). Provide this or 'content'."},
                    "content": {"type": "string", "description": "Inline Markdown/text to chunk instead of a file."},
                    "max_tokens": {"type": "integer", "description": "Max approximate tokens per chunk", "default": 512},
                    "overlap": {"type": "integer", "description": "Word overlap between adjacent chunks", "default": 64},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "extract_chunks_for_rag",
            "description": "Convert any supported file to Markdown then chunk it into JSON records ({id, text, metadata}) ready for embedding. Primary ingestion entry point for RAG pipelines.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to ingest (any supported format)."},
                    "content": {"type": "string", "description": "Inline content instead of a file."},
                    "max_tokens": {"type": "integer", "default": 512},
                    "overlap": {"type": "integer", "default": 64},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "json"}
                }
            }
        },
        {
            "name": "get_document_outline",
            "description": "Extract a nested heading outline (level/title/anchor/children) from a document. Dual output: Markdown list or JSON tree.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to outline (any supported format)."},
                    "content": {"type": "string", "description": "Inline content instead of a file."},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "search_content",
            "description": "Search inside converted document content across a directory, ranked by term frequency. Returns ranked snippets with source and score. Dual output: Markdown or JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {"type": "string", "description": "Directory to search (recursive)", "default": "."},
                    "query": {"type": "string", "description": "Search terms"},
                    "max_results": {"type": "integer", "default": 10},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_text_statistics",
            "description": "Word/vocabulary statistics for a file: total words, distinct words, vocabulary richness, sentence/paragraph counts, and a per-word frequency table. Dual output: Markdown or JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to analyze (any supported format)."},
                    "content": {"type": "string", "description": "Inline content instead of a file."},
                    "top_n": {"type": "integer", "description": "Number of top words to report", "default": 25},
                    "min_length": {"type": "integer", "description": "Ignore words shorter than this", "default": 1},
                    "stopwords": {"type": "boolean", "description": "Exclude common English stopwords", "default": false},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "get_corpus_statistics",
            "description": "Aggregate word statistics across a directory: per-document word/distinct counts plus corpus totals and global distinct-word count. Dual output: Markdown or JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {"type": "string", "description": "Directory to analyze (recursive)", "default": "."},
                    "top_n": {"type": "integer", "default": 25},
                    "stopwords": {"type": "boolean", "description": "Exclude common English stopwords", "default": true},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "extract_tags",
            "description": "Extract #tags and frontmatter tags. With 'directory' builds a vault-wide tag index {tag, count, files}; with 'file_path'/'content' returns tags for one note. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {"type": "string", "description": "Vault directory to build a tag index from"},
                    "file_path": {"type": "string", "description": "Single file to extract tags from"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "extract_keywords",
            "description": "Salient terms for a document via TF-IDF (pass 'directory' to use the vault as the IDF corpus). Answers 'what is this note about'. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to analyze"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "directory": {"type": "string", "description": "Optional corpus directory for IDF weighting"},
                    "top_n": {"type": "integer", "default": 10},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "find_related_notes",
            "description": "Find notes similar to a given note via cosine similarity over TF vectors, boosted by shared tags/links. Powers 'see also' and RAG neighbor expansion. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "The note to find neighbors for"},
                    "directory": {"type": "string", "description": "Vault directory to search", "default": "."},
                    "embeddings": {"type": "boolean", "description": "Use vector embeddings instead of TF/SimHash heuristics (persistent per-directory index; falls back to hashed vectors when no model is available)", "default": false},
                    "max_results": {"type": "integer", "default": 5},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "summarize_document",
            "description": "Extractive TL;DR: rank sentences by keyword density and position and return the top ones. Deterministic, no LLM. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to summarize"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "sentences": {"type": "integer", "description": "Number of sentences to return", "default": 3},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "extract_qa_pairs",
            "description": "Mine Q:/A: lines and '?'-terminated headings into {question, answer, source} pairs — flashcards, eval sets, or RAG ground-truth. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to mine"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "extract_entities",
            "description": "Lightweight entity extraction: URLs, emails, dates, and capitalized name phrases, aggregated with counts. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to analyze"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "build_knowledge_index",
            "description": "Flagship 'second brain' export: one JSON artifact bundling summary, outline, tags, keywords, stats, and RAG chunks for a document — a single object an AI agent or vector DB can ingest to 'know' the note. Always JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to index (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"}
                }
            }
        },
        {
            "name": "retrieve_context",
            "description": "RAG retrieval: rank chunks across a directory (or file) against a query and assemble the top ones into one context block under a token budget, with citations. The retrieval step to feed an LLM. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "The question/topic to retrieve context for"},
                    "directory": {"type": "string", "description": "Directory to search (recursive)", "default": "."},
                    "file_path": {"type": "string", "description": "Single file to retrieve from instead of a directory"},
                    "max_tokens": {"type": "integer", "description": "Context token budget", "default": 2000},
                    "top_k": {"type": "integer", "description": "Max number of chunks", "default": 8},
                    "embeddings": {"type": "boolean", "description": "Use vector embeddings instead of TF/SimHash heuristics (persistent per-directory index; falls back to hashed vectors when no model is available)", "default": false},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                },
                "required": ["query"]
            }
        },
        {
            "name": "count_tokens",
            "description": "Estimate token count for a file/content and show whether it fits each model's context window (Opus 4.8, Sonnet 5, Haiku 4.5). Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to measure (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "find_duplicates",
            "description": "Detect near-duplicate documents across a directory using SimHash. Returns groups of similar files with a similarity score. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {"type": "string", "description": "Directory to scan (recursive)", "default": "."},
                    "threshold": {"type": "integer", "description": "Max differing bits (0-64); lower is stricter (SimHash mode)", "default": 3},
                    "min_similarity": {"type": "number", "description": "Minimum cosine similarity to group (embeddings mode)", "default": 0.9},
                    "embeddings": {"type": "boolean", "description": "Use vector embeddings instead of TF/SimHash heuristics (persistent per-directory index; falls back to hashed vectors when no model is available)", "default": false},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "cluster_documents",
            "description": "Cluster documents in a directory by topic via cosine similarity over term vectors. Returns labeled groups with top terms. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": {"type": "string", "description": "Directory to cluster (recursive)", "default": "."},
                    "min_similarity": {"type": "number", "description": "Minimum cosine similarity to join a cluster", "default": 0.2},
                    "embeddings": {"type": "boolean", "description": "Use vector embeddings instead of TF/SimHash heuristics (persistent per-directory index; falls back to hashed vectors when no model is available)", "default": false},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "analyze_readability",
            "description": "Compute Flesch Reading Ease and Flesch-Kincaid grade level with word/sentence/syllable counts. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to analyze (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "detect_natural_language",
            "description": "Detect the natural language of a document (English/Spanish/French/German/Portuguese/Italian) via function-word analysis. Dual output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to analyze (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "top_n": {"type": "integer", "default": 3},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "classify_document",
            "description": "Heuristic topic/content-type classification (technical, finance, legal, correspondence, academic, marketing) via keyword signals. Dual output. See ai_classify for the LLM version.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to classify (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                }
            }
        },
        {
            "name": "ai_summarize",
            "description": "Abstractive summary via the Claude API (requires ANTHROPIC_API_KEY; returns a setup note if absent). Local alternative: summarize_document.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to summarize (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "style": {"type": "string", "description": "Summary style, e.g. concise, detailed, bullet points", "default": "concise"},
                    "model": {"type": "string", "description": "Override model (default claude-haiku-4-5)"},
                    "max_tokens": {"type": "integer", "default": 600}
                }
            }
        },
        {
            "name": "ai_ask",
            "description": "RAG question-answering via the Claude API: retrieves relevant context across a directory/file, then answers grounded in it with citations (requires ANTHROPIC_API_KEY).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": {"type": "string", "description": "The question to answer"},
                    "directory": {"type": "string", "description": "Directory to retrieve context from", "default": "."},
                    "file_path": {"type": "string", "description": "Single file to answer from instead of a directory"},
                    "model": {"type": "string", "description": "Override model (default claude-haiku-4-5)"},
                    "max_tokens": {"type": "integer", "default": 800},
                    "output_format": {"type": "string", "enum": ["markdown", "json"], "default": "markdown"}
                },
                "required": ["question"]
            }
        },
        {
            "name": "ai_tag",
            "description": "Suggest topical tags for a document via the Claude API (requires ANTHROPIC_API_KEY). Local alternative: extract_keywords.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to tag (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "max_tags": {"type": "integer", "default": 8},
                    "model": {"type": "string", "description": "Override model (default claude-haiku-4-5)"}
                }
            }
        },
        {
            "name": "ai_translate",
            "description": "Translate a document to a target language via the Claude API, preserving Markdown (requires ANTHROPIC_API_KEY).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to translate (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "target_language": {"type": "string", "description": "Target language, e.g. Spanish, French"},
                    "model": {"type": "string", "description": "Override model (default claude-haiku-4-5)"},
                    "max_tokens": {"type": "integer", "default": 2000}
                },
                "required": ["target_language"]
            }
        },
        {
            "name": "ai_classify",
            "description": "Classify a document into caller-provided labels via the Claude API (requires ANTHROPIC_API_KEY). Local alternative: classify_document.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string", "description": "File to classify (any supported format)"},
                    "content": {"type": "string", "description": "Inline content instead of a file"},
                    "labels": {"type": "array", "items": {"type": "string"}, "description": "Candidate labels"},
                    "model": {"type": "string", "description": "Override model (default claude-haiku-4-5)"}
                },
                "required": ["labels"]
            }
        },
        {
            "name": "browser_open_url",
            "description": "Open a URL in a real Chromium browser (executes JavaScript, keeps cookies/session). Headless by default; set visible=true to show the window so a human can solve CAPTCHAs, log in, or dismiss banners. The session stays open across tool calls — follow up with browser_capture_markdown to convert the rendered page, and browser_close when done.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to open (http:// or https://)"
                    },
                    "visible": {
                        "type": "boolean",
                        "description": "Show the browser window for human interaction (default: false = headless)",
                        "default": false
                    },
                    "wait_seconds": {
                        "type": "integer",
                        "description": "Extra seconds to wait after page load for JS-rendered content to settle (default: 0)",
                        "default": 0
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Navigation timeout in seconds (default: 30)",
                        "default": 30
                    },
                    "user_agent": {
                        "type": "string",
                        "description": "Optional custom User-Agent string"
                    }
                },
                "required": ["url"]
            }
        },
        {
            "name": "browser_capture_markdown",
            "description": "Capture the rendered HTML of the page currently open in the browser session (after any human interaction) and convert it to Markdown. Optionally pass a url to navigate first — with no prior session this opens a headless browser, so JS-heavy pages can be converted in a single call. Supports the same HTML processing options as convert_from_source.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Optional URL to navigate to before capturing (opens a headless session if none exists)"
                    },
                    "wait_seconds": {
                        "type": "integer",
                        "description": "Extra seconds to wait after navigation before capturing (default: 0)",
                        "default": 0
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Navigation timeout in seconds when url is given (default: 30)",
                        "default": 30
                    },
                    "extract_metadata": {
                        "type": "boolean",
                        "description": "Extract page metadata as YAML frontmatter (default: false)",
                        "default": false
                    },
                    "preserve_css_hints": {
                        "type": "boolean",
                        "description": "Preserve CSS styling hints as HTML comments (default: false)",
                        "default": false
                    },
                    "generate_toc": {
                        "type": "boolean",
                        "description": "Generate table of contents from headings (default: false)",
                        "default": false
                    },
                    "toc_max_level": {
                        "type": "integer",
                        "description": "Maximum heading level to include in TOC (1-6, default: 3)",
                        "default": 3
                    },
                    "extract_images": {
                        "type": "boolean",
                        "description": "Extract and process images (default: false)",
                        "default": false
                    },
                    "image_format": {
                        "type": "string",
                        "description": "Image output format: link, reference, or base64 (default: link)",
                        "default": "link"
                    },
                    "convert_tables": {
                        "type": "boolean",
                        "description": "Convert HTML tables to Markdown tables (default: false)",
                        "default": false
                    },
                    "extract_forms": {
                        "type": "boolean",
                        "description": "Extract form structure (default: false)",
                        "default": false
                    },
                    "preserve_comments": {
                        "type": "boolean",
                        "description": "Preserve HTML comments with summary (default: false)",
                        "default": false
                    },
                    "extract_links": {
                        "type": "boolean",
                        "description": "Extract link summary (default: false)",
                        "default": false
                    },
                    "analyze_headings": {
                        "type": "boolean",
                        "description": "Analyze heading structure (default: false)",
                        "default": false
                    },
                    "extract_definition_lists": {
                        "type": "boolean",
                        "description": "Extract definition lists (default: false)",
                        "default": false
                    }
                }
            }
        },
        {
            "name": "browser_close",
            "description": "Close the Chromium browser session opened by browser_open_url and free its resources.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "obsidian_vault_index",
            "description": "Index an Obsidian vault: note/attachment counts, tag frequencies, aliases, broken and ambiguous wikilinks, and optionally orphan notes. Understands [[link|alias]], [[note#heading]], [[note#^block]], ![[embeds]] and frontmatter aliases.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "include_orphans": {"type": "boolean", "description": "List notes with no inbound links (default: false)", "default": false}
                },
                "required": []
            }
        },
        {
            "name": "obsidian_get_note",
            "description": "Get a note by path, filename stem, or frontmatter alias: parsed YAML frontmatter, aliases, tags (frontmatter + inline #tags), headings, outgoing wikilinks, backlink count, and content. Optionally transclude ![[embeds]] recursively.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "note": {"type": "string", "description": "Note path, stem, or alias"},
                    "inline_embeds": {"type": "boolean", "description": "Replace ![[embeds]] with the embedded note content (default: false)", "default": false},
                    "max_embed_depth": {"type": "integer", "description": "Maximum transclusion recursion depth (default: 3)", "default": 3}
                },
                "required": ["note"]
            }
        },
        {
            "name": "obsidian_resolve_link",
            "description": "Resolve a wikilink string ([[target#heading|alias]], alias, path, or ^block form) using Obsidian's shortest-path rules. Reports Resolved/Ambiguous/Broken, whether the heading exists, and the block anchor line.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "link": {"type": "string", "description": "Link text, with or without [[ ]]"},
                    "from_note": {"type": "string", "description": "Source note for same-folder disambiguation (optional)"}
                },
                "required": ["link"]
            }
        },
        {
            "name": "obsidian_get_backlinks",
            "description": "List all inbound wikilinks to a note — including alias, heading, block, and embed forms — with the linking note and the source line as context.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "note": {"type": "string", "description": "Note path, stem, or alias"}
                },
                "required": ["note"]
            }
        },
        {
            "name": "obsidian_search",
            "description": "Search an Obsidian vault by tag (with nested-tag prefix matching), alias, frontmatter field (key or key=value), or full text.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "query": {"type": "string", "description": "Search query; for field mode use 'key' or 'key=value'"},
                    "mode": {"type": "string", "description": "One of: tag, alias, field, text", "enum": ["tag", "alias", "field", "text"]},
                    "limit": {"type": "integer", "description": "Maximum hits (default: 50)", "default": 50}
                },
                "required": ["query", "mode"]
            }
        },
        {
            "name": "obsidian_list_tasks",
            "description": "List checkbox tasks across the vault or one note, with all states ([ ] open, [x] done, [/] in progress, [-] cancelled, [>] forwarded, ...), nesting depth, #tags, and Tasks-plugin dates (📅 due, ✅ done, ⏳ scheduled, 🛫 start, 🔁 recurrence).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "status": {"type": "string", "description": "Filter: open, done, in_progress, cancelled, forwarded, or a raw state char"},
                    "note": {"type": "string", "description": "Limit to one note (path, stem, or alias)"}
                },
                "required": []
            }
        },
        {
            "name": "obsidian_get_vault_config",
            "description": "Read the vault's .obsidian configuration: attachment folder, new-link format, daily-notes folder/format/template, templates folder, and enabled core plugins.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"}
                },
                "required": []
            }
        },
        {
            "name": "obsidian_create_note_from_template",
            "description": "Create a note (or today's daily note per .obsidian/daily-notes.json) from a template, substituting {{title}}, {{date}}, {{time}}, and {{date:FORMAT}}. Refuses to overwrite existing notes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "title": {"type": "string", "description": "Note title, optionally with folder ('projects/New Idea'). Required unless daily=true"},
                    "template": {"type": "string", "description": "Vault-relative template path (default: daily-notes template when daily=true)"},
                    "daily": {"type": "boolean", "description": "Create today's daily note using the vault's daily-notes settings (default: false)", "default": false}
                },
                "required": []
            }
        },
        {
            "name": "obsidian_rename_note",
            "description": "Rename or move a note and rewrite every inbound wikilink (preserving |alias, #heading, and #^block fragments), like Obsidian's own rename. dry_run defaults to true — pass dry_run=false to apply.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "note": {"type": "string", "description": "Note to rename (path, stem, or alias)"},
                    "new_name": {"type": "string", "description": "New stem (same folder) or vault-relative path"},
                    "dry_run": {"type": "boolean", "description": "Preview changes without writing (default: true)", "default": true}
                },
                "required": ["note", "new_name"]
            }
        },
        {
            "name": "obsidian_convert_canvas",
            "description": "Convert an Obsidian .canvas file (JsonCanvas: text/file/link/group nodes and edges) to structured Markdown with groups, nodes in reading order, and connections.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "canvas_path": {"type": "string", "description": "Path to the .canvas file"}
                },
                "required": ["canvas_path"]
            }
        },
        {
            "name": "analyze_text",
            "description": "Complete text metrics: word/character/space/token counts plus sorted frequency tables for words, characters, and tokens. Modular provider-aware tokenization — openai (tiktoken, exact), anthropic (cl100k proxy, estimate), meta/llama/qwen/deepseek (exact with a tokenizer.json via tokenizer_file, else estimate), grok (estimate), heuristic. Estimated counts are clearly flagged.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "Inline text to analyze"},
                    "file_path": {"type": "string", "description": "File to analyze instead of inline content"},
                    "provider": {"type": "string", "description": "Tokenizer provider: openai, anthropic, meta, llama, qwen, deepseek, grok, heuristic (default: anthropic)", "default": "anthropic"},
                    "model": {"type": "string", "description": "Model hint for the provider (e.g. gpt-4o vs gpt-4 selects o200k vs cl100k)"},
                    "tokenizer_file": {"type": "string", "description": "Path to a HuggingFace tokenizer.json for exact meta/qwen/deepseek counts"},
                    "top": {"type": "integer", "description": "Rows per frequency table (default 50; 0 = all)", "default": 50},
                    "output_format": {"type": "string", "description": "markdown (default) or json", "default": "markdown"}
                }
            }
        },
        {
            "name": "obsidian_extract_dataview_fields",
            "description": "Extract Dataview fields — inline 'key:: value' (line, [bracketed], (parenthesized) forms) and frontmatter properties — across the vault or one note, optionally filtered by field name.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault_path": {"type": "string", "description": "Path to the vault root directory. Optional when the server runs with --base-dir (defaults to the first base dir)"},
                    "note": {"type": "string", "description": "Limit to one note (path, stem, or alias)"},
                    "field": {"type": "string", "description": "Only this field name (case-insensitive)"}
                },
                "required": []
            }
        }
    ])
}

fn handle_list_tools(id: &Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: id.clone(),
        result: Some(json!({
            "tools": tool_definitions()
        })),
        error: None,
    }
}

fn rpc_result(id: Value, result: Value) -> JsonRpcResponse {
    JsonRpcResponse { jsonrpc: "2.0".to_string(), id, result: Some(result), error: None }
}

fn rpc_error(id: Value, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError { code, message, data: None }),
    }
}

/// Cap on resources/list output so a huge vault can't flood the client.
const MAX_RESOURCES: usize = 1000;

/// Collect the files of one base dir as MCP resource descriptors.
fn collect_resources(dir: &std::path::Path, out: &mut Vec<Value>) {
    let Ok(files) = list_files_in_directory(&dir.to_string_lossy(), true) else {
        return;
    };
    for f in files {
        if out.len() >= MAX_RESOURCES {
            return;
        }
        let name = f.strip_prefix(dir).unwrap_or(&f).to_string_lossy().into_owned();
        let mime = if f.extension().is_some_and(|e| e == "md") {
            "text/markdown".to_string()
        } else {
            mime_guess::from_path(&f).first_raw().unwrap_or("application/octet-stream").to_string()
        };
        out.push(json!({
            "uri": format!("file://{}", f.display()),
            "name": name,
            "mimeType": mime,
        }));
    }
}

/// MCP resources/list: files under the --base-dir vault(s) as file:// URIs.
/// Empty when the server was started without --base-dir.
fn handle_list_resources(id: &Value) -> JsonRpcResponse {
    let mut resources = Vec::new();
    for dir in base_dirs() {
        collect_resources(dir, &mut resources);
    }
    rpc_result(id.clone(), json!({ "resources": resources }))
}

/// Resolve a file:// URI to a path, restricted to the --base-dir vault(s).
fn resource_path_in(dirs: &[std::path::PathBuf], uri: &str) -> Result<std::path::PathBuf, String> {
    let raw = uri
        .strip_prefix("file://")
        .ok_or_else(|| format!("Unsupported URI scheme: {}", uri))?;
    if dirs.is_empty() {
        return Err("Resources unavailable: server started without --base-dir".to_string());
    }
    let path = std::path::Path::new(raw)
        .canonicalize()
        .map_err(|e| format!("Resource not found: {} ({})", raw, e))?;
    if !dirs.iter().any(|d| path.starts_with(d)) {
        return Err(format!("Resource outside the configured base directories: {}", raw));
    }
    Ok(path)
}

/// MCP resources/read: return a vault file's content. Markdown is returned
/// verbatim; every other supported format is converted to Markdown first.
fn handle_read_resource(request: &JsonRpcRequest) -> JsonRpcResponse {
    let id = request_id(request);
    let Some(uri) = request.params.get("uri").and_then(Value::as_str) else {
        return rpc_error(id, -32602, "Missing required parameter: uri".to_string());
    };
    let path = match resource_path_in(base_dirs(), uri) {
        Ok(p) => p,
        Err(msg) => return rpc_error(id, -32602, msg),
    };
    let text = if path.extension().is_some_and(|e| e == "md") {
        match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return rpc_error(id, -32603, format!("Failed to read {}: {}", path.display(), e)),
        }
    } else {
        let args = json!({"file_path": path.to_string_lossy(), "include_filename": false});
        match handle_convert_file(&args) {
            Ok(t) => t,
            Err(e) => return rpc_error(id, -32603, format!("Failed to convert {}: {}", path.display(), e)),
        }
    };
    rpc_result(id, json!({
        "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": text }]
    }))
}

/// MCP prompts/list: reusable prompt templates over this server's tools.
fn prompt_definitions() -> Value {
    json!([
        {
            "name": "summarize_note",
            "description": "Read and summarize a note or document from the vault",
            "arguments": [
                {"name": "path", "description": "Path to the note or document", "required": true}
            ]
        },
        {
            "name": "ingest_url",
            "description": "Capture a web page as Markdown and optionally save it into the vault",
            "arguments": [
                {"name": "url", "description": "Page URL to capture", "required": true},
                {"name": "save_path", "description": "Vault-relative path to save the captured Markdown", "required": false}
            ]
        },
        {
            "name": "vault_health",
            "description": "Audit the vault: broken links, orphan notes, and near-duplicates",
            "arguments": [
                {"name": "vault_path", "description": "Vault root (omit to use the server's base dir)", "required": false}
            ]
        }
    ])
}

fn handle_list_prompts(id: &Value) -> JsonRpcResponse {
    rpc_result(id.clone(), json!({ "prompts": prompt_definitions() }))
}

/// Render one prompt template with its arguments. Returns (description, text).
fn render_prompt(name: &str, args: &Value) -> Result<(String, String), String> {
    let arg = |key: &str| args.get(key).and_then(Value::as_str);
    match name {
        "summarize_note" => {
            let path = arg("path").ok_or("Missing required argument: path")?;
            Ok((
                "Summarize a vault note or document".to_string(),
                format!(
                    "Using the toMarkdownMCP tools, read and summarize the document at '{}'. \
                     Call get_file_summary first for structure, then summarize_document \
                     (or ai_summarize when an API key is configured). Present a concise \
                     summary followed by the key points as bullets.",
                    path
                ),
            ))
        }
        "ingest_url" => {
            let url = arg("url").ok_or("Missing required argument: url")?;
            let mut text = format!(
                "Capture the web page {} as Markdown using the browser_capture_markdown tool \
                 (pass the url directly; set extract_metadata to true).",
                url
            );
            if let Some(save) = arg("save_path") {
                text.push_str(&format!(
                    " Then save the captured Markdown to '{}' with create_or_append_file.",
                    save
                ));
            }
            Ok(("Capture a web page into Markdown".to_string(), text))
        }
        "vault_health" => {
            let target = arg("vault_path")
                .map(|p| format!("the vault at '{}'", p))
                .unwrap_or_else(|| "the configured vault".to_string());
            Ok((
                "Audit vault link and note health".to_string(),
                format!(
                    "Audit {} using obsidian_vault_index with include_orphans set to true, \
                     then find_duplicates on the same directory. Report broken links, \
                     ambiguous links, orphan notes, and near-duplicate groups, each with a \
                     suggested fix.",
                    target
                ),
            ))
        }
        other => Err(format!("Unknown prompt: {}", other)),
    }
}

fn handle_get_prompt(request: &JsonRpcRequest) -> JsonRpcResponse {
    let id = request_id(request);
    let Some(name) = request.params.get("name").and_then(Value::as_str) else {
        return rpc_error(id, -32602, "Missing required parameter: name".to_string());
    };
    let args = request.params.get("arguments").cloned().unwrap_or(json!({}));
    match render_prompt(name, &args) {
        Ok((description, text)) => rpc_result(id, json!({
            "description": description,
            "messages": [
                { "role": "user", "content": { "type": "text", "text": text } }
            ]
        })),
        Err(msg) => rpc_error(id, -32602, msg),
    }
}

/// String argument keys that hold filesystem paths and should resolve
/// against `--base-dir` directories when relative.
const PATH_KEYS: &[&str] = &[
    "file_path", "path", "vault_path", "directory", "old_path", "new_path", "tokenizer_file",
];
/// Directory-type keys that default to the primary base dir when omitted.
const INJECT_KEYS: &[&str] = &["vault_path", "directory"];

/// Resolve one path string against the given base dirs: absolute paths
/// pass through; relative paths pick the first base dir where they exist,
/// falling back to the primary base dir (so new files land somewhere sane).
fn resolve_with(dirs: &[std::path::PathBuf], value: &str) -> String {
    let expanded = expand_tilde(value);
    let p = std::path::Path::new(&expanded);
    if p.is_absolute() || dirs.is_empty() {
        return expanded;
    }
    // Don't touch URLs or the stdin sentinel.
    if expanded.contains("://") || expanded == "-" {
        return expanded;
    }
    for dir in dirs {
        let candidate = dir.join(p);
        if candidate.exists() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    dirs[0].join(p).to_string_lossy().into_owned()
}

/// Rewrite path-like arguments in place per the `--base-dir` rules.
fn apply_base_dirs_with(dirs: &[std::path::PathBuf], arguments: &mut Value) {
    if dirs.is_empty() {
        return;
    }
    let Some(obj) = arguments.as_object_mut() else { return };
    for key in PATH_KEYS {
        if let Some(Value::String(s)) = obj.get(*key) {
            let resolved = resolve_with(dirs, s);
            obj.insert((*key).to_string(), Value::String(resolved));
        }
    }
    if let Some(Value::Array(items)) = obj.get_mut("file_paths") {
        for item in items.iter_mut() {
            if let Value::String(s) = item {
                *item = Value::String(resolve_with(dirs, s));
            }
        }
    }
    // `source` is path-or-URL: resolve only when it looks like a local path.
    if let Some(Value::String(s)) = obj.get("source") {
        if !s.contains("://") && s != "-" {
            let resolved = resolve_with(dirs, s);
            obj.insert("source".to_string(), Value::String(resolved));
        }
    }
    for key in INJECT_KEYS {
        if !obj.contains_key(*key) {
            obj.insert(
                (*key).to_string(),
                Value::String(dirs[0].to_string_lossy().into_owned()),
            );
        }
    }
}

/// No-op when the server was started without `--base-dir`.
fn apply_base_dirs(arguments: &mut Value) {
    apply_base_dirs_with(base_dirs(), arguments);
}

async fn handle_call_tool(request: &JsonRpcRequest) -> JsonRpcResponse {
    let tool_name = request.params.get("name").and_then(|v| v.as_str());
    let mut arguments = request.params.get("arguments").cloned().unwrap_or(Value::Object(Default::default()));
    apply_base_dirs(&mut arguments);

    let result = match tool_name {
        Some("convert_file") => handle_convert_file(&arguments),
        Some("convert_text") => handle_convert_text(&arguments),
        Some("convert_from_source") => {
            // This needs to be async
            match handle_convert_from_source(&arguments).await {
                Ok(content) => Ok(content),
                Err(e) => Err(e),
            }
        }
        Some("list_directory_files") => handle_list_directory_files(&arguments),
        Some("get_file_summary") => handle_get_file_summary(&arguments),
        Some("batch_convert_files") => handle_batch_convert_files(&arguments),
        Some("search_files") => handle_search_files(&arguments),
        Some("get_recently_modified_files") => handle_get_recently_modified_files(&arguments),
        Some("get_vault_statistics") => handle_get_vault_statistics(&arguments),
        Some("extract_active_todos") => handle_extract_active_todos(&arguments),
        Some("safe_append_or_replace_section") => handle_safe_append_or_replace_section(&arguments),
        Some("resolve_and_validate_links") => handle_resolve_and_validate_links(&arguments),
        Some("upsert_markdown_table") => handle_upsert_markdown_table(&arguments),
        Some("read_file") => handle_read_file(&arguments),
        Some("create_or_append_file") => handle_create_or_append_file(&arguments),
        Some("get_tool_help") => handle_get_tool_help(&arguments),
        Some("move_or_rename_file") => handle_move_or_rename_file(&arguments),
        Some("delete_file") => handle_delete_file(&arguments),
        Some("batch_create_notes") => handle_batch_create_notes(&arguments),
        Some("update_note_properties") => handle_update_note_properties(&arguments),
        Some("find_note_by_alias_or_title") => handle_find_note_by_alias_or_title(&arguments),
        Some("get_graph_relationships") => handle_get_graph_relationships(&arguments),
        Some("chunk_markdown") => handle_chunk_markdown(&arguments),
        Some("extract_chunks_for_rag") => handle_extract_chunks_for_rag(&arguments),
        Some("get_document_outline") => handle_get_document_outline(&arguments),
        Some("search_content") => handle_search_content(&arguments),
        Some("get_text_statistics") => handle_get_text_statistics(&arguments),
        Some("get_corpus_statistics") => handle_get_corpus_statistics(&arguments),
        Some("extract_tags") => handle_extract_tags(&arguments),
        Some("extract_keywords") => handle_extract_keywords(&arguments),
        Some("find_related_notes") => handle_find_related_notes(&arguments),
        Some("summarize_document") => handle_summarize_document(&arguments),
        Some("extract_qa_pairs") => handle_extract_qa_pairs(&arguments),
        Some("extract_entities") => handle_extract_entities(&arguments),
        Some("build_knowledge_index") => handle_build_knowledge_index(&arguments),
        Some("retrieve_context") => handle_retrieve_context(&arguments),
        Some("count_tokens") => handle_count_tokens(&arguments),
        Some("find_duplicates") => handle_find_duplicates(&arguments),
        Some("cluster_documents") => handle_cluster_documents(&arguments),
        Some("analyze_readability") => handle_analyze_readability(&arguments),
        Some("detect_natural_language") => handle_detect_natural_language(&arguments),
        Some("classify_document") => handle_classify_document(&arguments),
        Some("ai_summarize") => handle_ai_summarize(&arguments).await,
        Some("ai_ask") => handle_ai_ask(&arguments).await,
        Some("ai_tag") => handle_ai_tag(&arguments).await,
        Some("ai_translate") => handle_ai_translate(&arguments).await,
        Some("ai_classify") => handle_ai_classify(&arguments).await,
        Some("browser_open_url") => handle_browser_open_url(&arguments).await,
        Some("browser_capture_markdown") => handle_browser_capture_markdown(&arguments).await,
        Some("browser_close") => handle_browser_close(&arguments).await,
        Some("obsidian_vault_index") => handle_obsidian_vault_index(&arguments),
        Some("obsidian_get_note") => handle_obsidian_get_note(&arguments),
        Some("obsidian_resolve_link") => handle_obsidian_resolve_link(&arguments),
        Some("obsidian_get_backlinks") => handle_obsidian_get_backlinks(&arguments),
        Some("obsidian_search") => handle_obsidian_search(&arguments),
        Some("obsidian_list_tasks") => handle_obsidian_list_tasks(&arguments),
        Some("obsidian_get_vault_config") => handle_obsidian_get_vault_config(&arguments),
        Some("obsidian_create_note_from_template") => handle_obsidian_create_note_from_template(&arguments),
        Some("obsidian_rename_note") => handle_obsidian_rename_note(&arguments),
        Some("obsidian_convert_canvas") => handle_obsidian_convert_canvas(&arguments),
        Some("obsidian_extract_dataview_fields") => handle_obsidian_extract_dataview_fields(&arguments),
        Some("analyze_text") => handle_analyze_text(&arguments),
        _ => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id(request),
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: "Invalid params".to_string(),
                    data: Some(json!("Unknown tool")),
                }),
            };
        }
    };

    match result {
        Ok(content) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request_id(request),
            result: Some(json!({
                "content": [
                    {
                        "type": "text",
                        "text": content
                    }
                ]
            })),
            error: None,
        },
        Err(e) => {
            // JSON-RPC error taxonomy: bad/missing arguments are the
            // caller's fault (-32602 Invalid params); everything else is a
            // failed execution (-32603) with the real cause as the message.
            let msg = e.to_string();
            let code = match e.downcast_ref::<ConversionError>() {
                Some(ConversionError::MissingParameter(_)) => -32602,
                _ => -32603,
            };
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id(request),
                result: None,
                error: Some(JsonRpcError {
                    code,
                    message: msg.clone(),
                    data: Some(json!(msg)),
                }),
            }
        }
    }
}

/// Above this size, structured formats are refused (their parsers hold the
/// whole transformed document in memory, typically several times the input)
/// and plain text/code switches to a single-pass buffered read. The
/// `max_bytes` tool parameter overrides it.
const LARGE_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// True when the extension belongs to a structured format whose converter
/// must load and transform the entire file (HTML family, markup, binary docs).
fn is_structured_ext(ext: Option<&str>) -> bool {
    let Some(ext) = ext else { return false };
    matches!(ext, "html" | "htm" | "mhtml" | "webarchive")
        || markup_converter::is_markup_extension(ext)
        || document_converter::is_document_extension(ext)
        || office_converter::is_office_extension(ext)
        || feed_email_converter::is_feed_email_extension(ext)
}

fn large_file_error(path: &str, size: u64, limit: u64) -> Box<dyn std::error::Error> {
    Box::new(ConversionError::ConversionFailed(format!(
        "{} is {:.1} MB, above the {:.0} MB limit for structured conversion. \
         Use get_file_summary for an overview, chunk_markdown/extract_chunks_for_rag \
         for piecewise processing, or pass max_bytes to raise the limit.",
        path,
        size as f64 / (1024.0 * 1024.0),
        limit as f64 / (1024.0 * 1024.0),
    )))
}

/// Single-pass fenced-code conversion for large plain-text files: one
/// pre-sized output buffer, streamed line reads, no intermediate copies.
fn convert_large_text_file(
    path: &Path,
    language: &str,
    title: &str,
    add_line_numbers: bool,
    size_hint: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
    let mut reader = std::io::BufReader::with_capacity(1 << 20, file);
    let mut out = String::with_capacity(size_hint as usize + (size_hint / 8) as usize + 128);
    if !title.is_empty() {
        out.push_str(&format!("# {}\n\n", title));
    }
    out.push_str("```");
    out.push_str(language);
    out.push('\n');
    let mut line = String::new();
    let mut n = 0usize;
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
        if read == 0 {
            break;
        }
        if add_line_numbers {
            n += 1;
            out.push_str(&format!("{:4} | ", n));
            out.push_str(line.trim_end_matches(['\n', '\r']));
            out.push('\n');
        } else {
            out.push_str(&line);
        }
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("```\n");
    Ok(out)
}

/// If the extension is a binary/office document format, convert it to Markdown.
/// Returns None for formats handled by the normal text pipeline.
fn try_convert_binary_document(path: &Path) -> Option<Result<String, Box<dyn std::error::Error>>> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    let map_err = |e: anyhow::Error| {
        Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>
    };
    if document_converter::is_document_extension(ext) {
        Some(document_converter::convert_document(path).map_err(map_err))
    } else if office_converter::is_office_extension(ext) {
        Some(office_converter::convert_office(path).map_err(map_err))
    } else if feed_email_converter::is_feed_email_extension(ext) {
        Some(feed_email_converter::convert_feed_email(path).map_err(map_err))
    } else {
        None
    }
}

/// Convert any supported file to Markdown/plain text for RAG/analysis. Unlike
/// `handle_convert_file`, code/text files are returned as-is (no code fence).
fn convert_any_to_markdown(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size > LARGE_FILE_BYTES && is_structured_ext(path.extension().and_then(|e| e.to_str())) {
        return Err(large_file_error(&path.display().to_string(), size, LARGE_FILE_BYTES));
    }
    if let Some(conv) = try_convert_binary_document(path) {
        return conv;
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if markup_converter::is_markup_extension(ext) {
            return Ok(markup_converter::convert_markup(ext, &content));
        }
    }
    Ok(content)
}

/// Resolve text content from either an inline `content` arg or a `file_path`
/// arg (which is converted to Markdown). Used by the RAG/knowledge tools.
fn resolve_content_arg(args: &Value) -> Result<(String, String), Box<dyn std::error::Error>> {
    if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
        return Ok((content.to_string(), "inline".to_string()));
    }
    if let Some(file_path) = args.get("file_path").and_then(|v| v.as_str()) {
        let md = convert_any_to_markdown(Path::new(file_path))?;
        return Ok((md, file_path.to_string()));
    }
    Err(Box::new(ConversionError::MissingParameter(
        "file_path or content".to_string(),
    )) as Box<dyn std::error::Error>)
}

/// Prepend a `# filename` heading to converted content when a filename is given.
fn prepend_filename_heading(filename: &str, body: String) -> String {
    if filename.is_empty() {
        body
    } else {
        format!("# {}\n\n{}", filename, body)
    }
}

fn handle_convert_file(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let file_path = args.get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("file_path".to_string())) as Box<dyn std::error::Error>)?;

    let include_filename = args.get("include_filename")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let explicit_file_type = args.get("file_type")
        .and_then(|v| v.as_str());

    let add_line_numbers = args.get("add_line_numbers")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_metadata = args.get("extract_metadata")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let preserve_css_hints = args.get("preserve_css_hints")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let generate_toc_flag = args.get("generate_toc")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let toc_max_level = args.get("toc_max_level")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as usize;

    let extract_images = args.get("extract_images")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let image_format_str = args.get("image_format")
        .and_then(|v| v.as_str())
        .unwrap_or("link");

    let image_format = ImageFormat::from_str(image_format_str)
        .unwrap_or(ImageFormat::Link);

    let convert_tables = args.get("convert_tables")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_forms = args.get("extract_forms")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let preserve_comments = args.get("preserve_comments")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_links = args.get("extract_links")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let analyze_headings = args.get("analyze_headings")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_definition_lists = args.get("extract_definition_lists")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_blockquotes = args.get("extract_blockquotes")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let path = Path::new(file_path);

    // Get filename if needed
    let filename = if include_filename {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("File")
    } else {
        ""
    };

    // Check if this is an HTML-like file
    let extension = path.extension().and_then(|ext| ext.to_str());
    let is_html = matches!(extension, Some("html" | "htm" | "mhtml" | "webarchive"));

    // Large-file gate: refuse structured formats above the limit, stream
    // plain text/code with a single pre-sized buffer instead.
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let size_limit = args.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(LARGE_FILE_BYTES);
    if file_size > size_limit {
        if is_structured_ext(extension) {
            return Err(large_file_error(file_path, file_size, size_limit));
        }
        let language = if let Some(explicit) = explicit_file_type {
            explicit.to_string()
        } else {
            let detected = detect_language(path);
            if detected.is_empty() {
                detect_language_from_filename(path.file_name().and_then(|n| n.to_str()).unwrap_or(""))
            } else {
                detected
            }
        };
        return convert_large_text_file(path, &language, filename, add_line_numbers, file_size);
    }

    if is_html {
        // Handle HTML conversion
        return handle_html_file_conversion(file_path, filename, extract_metadata, preserve_css_hints, generate_toc_flag, toc_max_level, extract_images, image_format, convert_tables, extract_forms, preserve_comments, extract_links, analyze_headings, extract_definition_lists, extract_blockquotes);
    }

    // Handle binary/office/document/feed/email formats (routed before the text
    // read below, since these are binary and would fail read_to_string).
    if let Some(conv) = try_convert_binary_document(path) {
        return Ok(prepend_filename_heading(filename, conv?));
    }

    // Read file
    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    // Lightweight-markup text formats (wiki, rST, AsciiDoc, Org, LaTeX, Textile)
    // convert to real Markdown instead of being wrapped in a code fence.
    if let Some(ext) = extension {
        if markup_converter::is_markup_extension(ext) {
            let md = markup_converter::convert_markup(ext, &content);
            return Ok(prepend_filename_heading(filename, md));
        }
    }

    // Detect language from file extension or use explicit type
    let language = if let Some(explicit) = explicit_file_type {
        explicit.to_string()
    } else {
        let detected = detect_language(path);
        if detected.is_empty() {
            detect_language_from_filename(
                path.file_name().and_then(|n| n.to_str()).unwrap_or("")
            )
        } else {
            detected
        }
    };

    Ok(convert_to_markdown_with_options(
        &content,
        Some(&language),
        Some(filename),
        add_line_numbers,
    ))
}

fn handle_convert_text(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let content = args.get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("content".to_string())) as Box<dyn std::error::Error>)?;

    let file_type = args.get("file_type")
        .and_then(|v| v.as_str());

    let title = args.get("title")
        .and_then(|v| v.as_str());

    let add_line_numbers = args.get("add_line_numbers")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(convert_to_markdown_with_options(
        content,
        file_type,
        title,
        add_line_numbers,
    ))
}

async fn handle_convert_from_source(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let source_str = args.get("source")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("source".to_string())) as Box<dyn std::error::Error>)?;

    let explicit_file_type = args.get("file_type")
        .and_then(|v| v.as_str());

    let title = args.get("title")
        .and_then(|v| v.as_str());

    let add_line_numbers = args.get("add_line_numbers")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_metadata = args.get("extract_metadata")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let preserve_css_hints = args.get("preserve_css_hints")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let generate_toc_flag = args.get("generate_toc")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let toc_max_level = args.get("toc_max_level")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as usize;

    let extract_images = args.get("extract_images")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let convert_tables = args.get("convert_tables")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_forms = args.get("extract_forms")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let preserve_comments = args.get("preserve_comments")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_links = args.get("extract_links")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let analyze_headings = args.get("analyze_headings")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_definition_lists = args.get("extract_definition_lists")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Parse source
    let source = SourceType::from_string(source_str)
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    // Fetch content
    let content = fetch_from_source(&source)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    // RSS/Atom feeds: detect by content sniffing (or explicit file_type) and
    // route to the feed converter regardless of source (URL, file, or stdin).
    let looks_like_feed = matches!(explicit_file_type, Some("rss" | "atom" | "feed"))
        || {
            let head = content.trim_start();
            let head = head.strip_prefix("\u{feff}").unwrap_or(head); // BOM
            let lower = head.get(..512.min(head.len())).unwrap_or(head).to_lowercase();
            lower.contains("<rss") || lower.contains("<feed") || lower.contains("<rdf:rdf")
        };
    if looks_like_feed {
        if let Ok(md) = feed_email_converter::feed_bytes_to_markdown(content.as_bytes()) {
            return Ok(md);
        }
    }

    // Detect language
    let language = if let Some(explicit) = explicit_file_type {
        explicit.to_string()
    } else {
        match &source {
            SourceType::FilePath(path) => {
                let p = Path::new(path);
                let detected = detect_language(p);
                if detected.is_empty() {
                    detect_language_from_filename(
                        p.file_name().and_then(|n| n.to_str()).unwrap_or("")
                    )
                } else {
                    detected
                }
            }
            SourceType::Url(url) => {
                // Try to detect from URL path
                if let Ok(parsed_url) = url::Url::parse(url) {
                    let path = parsed_url.path();
                    let p = Path::new(path);
                    detect_language(p)
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    };

    // Check if this is HTML and should use HTML-specific conversion
    let mut markdown = if language == "html" || language == "htm" {
        if extract_metadata || preserve_css_hints || extract_images || convert_tables || extract_forms || preserve_comments || extract_links || analyze_headings || extract_definition_lists {
            convert_html_with_flags(&content, args)?
        } else {
            convert_to_markdown_with_options(
                &content,
                Some(language.as_str()),
                title,
                add_line_numbers,
            )
        }
    } else {
        convert_to_markdown_with_options(
            &content,
            Some(language.as_str()),
            title,
            add_line_numbers,
        )
    };

    // Generate table of contents if requested
    if generate_toc_flag {
        markdown = generate_toc_for_markdown(&markdown, toc_max_level)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    }

    Ok(markdown)
}

/// Convert HTML to Markdown applying the optional HTML processing flags read
/// from `args` (extract_metadata, convert_tables, extract_links, ...). With no
/// flags set this reduces to a plain HTML → Markdown conversion. TOC
/// generation is handled by callers.
fn convert_html_with_flags(content: &str, args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let flag = |name: &str| args.get(name).and_then(|v| v.as_bool()).unwrap_or(false);

    let extract_metadata = flag("extract_metadata");
    let preserve_css_hints = flag("preserve_css_hints");
    let extract_images = flag("extract_images");
    let convert_tables = flag("convert_tables");
    let extract_forms = flag("extract_forms");
    let preserve_comments = flag("preserve_comments");
    let extract_links = flag("extract_links");
    let analyze_headings = flag("analyze_headings");
    let extract_definition_lists = flag("extract_definition_lists");

    let image_format_str = args.get("image_format")
        .and_then(|v| v.as_str())
        .unwrap_or("link");
    let image_format = ImageFormat::from_str(image_format_str)
        .unwrap_or(ImageFormat::Link);

    let mut html_content = content.to_string();
    let mut comment_summary = String::new();
    let mut link_summary = String::new();
    let mut heading_summary = String::new();
    let mut definition_list_summary = String::new();

    // Extract definition lists if needed
    if extract_definition_lists {
        let lists = definition_list_converter::extract_definition_lists_from_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        if !lists.is_empty() {
            definition_list_summary = definition_list_converter::generate_definition_list_summary(&lists);
        }
    }

    // Analyze headings if needed
    if analyze_headings {
        let headings = heading_analyzer::extract_headings_from_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        if !headings.is_empty() {
            let stats = heading_analyzer::analyze_heading_structure(&headings);
            heading_summary.push_str(&heading_analyzer::generate_heading_summary(&stats));
            heading_summary.push_str(&heading_analyzer::generate_heading_tree(&headings));
        }
    }

    // Extract links if needed
    if extract_links {
        let links = link_extractor::extract_links_from_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        if !links.is_empty() {
            link_summary = link_extractor::generate_link_summary(&links);
        }
    }

    // Process comments if needed (remove but extract info, or preserve)
    if preserve_comments {
        let (html, comments) = comment_extractor::process_comments_in_html(&html_content, true)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        html_content = html;
        if !comments.is_empty() {
            comment_summary = comment_extractor::generate_comment_summary(&comments);
        }
    }

    // Extract forms if needed
    if extract_forms {
        html_content = form_extractor::process_forms_in_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?
            .0;
    }

    // Convert tables if needed
    if convert_tables {
        html_content = table_converter::convert_tables_in_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    }

    // Process images if needed
    if extract_images {
        html_content = image_extractor::process_images_in_html(&html_content, image_format)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    }

    let html_markdown = html_to_markdown_with_options(&html_content, extract_metadata, preserve_css_hints)
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    // Prepend summaries if present (definition lists, headings, links, comments)
    if !definition_list_summary.is_empty() || !heading_summary.is_empty() || !link_summary.is_empty() || !comment_summary.is_empty() {
        let mut prefixed = String::new();
        if !definition_list_summary.is_empty() {
            prefixed.push_str(&definition_list_summary);
            prefixed.push('\n');
        }
        if !heading_summary.is_empty() {
            prefixed.push_str(&heading_summary);
            prefixed.push('\n');
        }
        if !link_summary.is_empty() {
            prefixed.push_str(&link_summary);
            prefixed.push('\n');
        }
        if !comment_summary.is_empty() {
            prefixed.push_str(&comment_summary);
            prefixed.push('\n');
        }
        prefixed.push_str(&html_markdown);
        Ok(prefixed)
    } else {
        Ok(html_markdown)
    }
}

async fn handle_browser_open_url(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let url = args.get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("url".to_string())) as Box<dyn std::error::Error>)?;

    let visible = args.get("visible")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let wait_seconds = args.get("wait_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let timeout_seconds = args.get("timeout_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(browser::DEFAULT_NAV_TIMEOUT_SECS);

    let user_agent = args.get("user_agent")
        .and_then(|v| v.as_str());

    let info = browser::open(url, visible, wait_seconds, user_agent, timeout_seconds)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    let mut result = String::new();
    result.push_str("# Browser Session Opened\n\n");
    result.push_str(&format!("- **URL:** {}\n", info.url));
    if let Some(title) = &info.title {
        result.push_str(&format!("- **Title:** {}\n", title));
    }
    result.push_str(&format!("- **Mode:** {}\n\n", if visible { "visible window" } else { "headless" }));
    if visible {
        result.push_str("The browser window is open. Interact with the page as needed (login, CAPTCHA, cookie banners), then call `browser_capture_markdown` to convert the rendered page. Call `browser_close` when finished.\n");
    } else {
        result.push_str("The page is loaded. Call `browser_capture_markdown` to convert the rendered page, and `browser_close` when finished.\n");
    }
    Ok(result)
}

async fn handle_browser_capture_markdown(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let navigate_to = args.get("url")
        .and_then(|v| v.as_str());

    let wait_seconds = args.get("wait_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let timeout_seconds = args.get("timeout_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(browser::DEFAULT_NAV_TIMEOUT_SECS);

    let (html, _info) = browser::capture_html(navigate_to, wait_seconds, timeout_seconds)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    // Rendered pages always carry inline scripts/styles; drop them so their
    // text doesn't leak into the Markdown.
    let html = html_converter::strip_non_content_tags(&html);

    let mut markdown = convert_html_with_flags(&html, args)?;

    let generate_toc_flag = args.get("generate_toc")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if generate_toc_flag {
        let toc_max_level = args.get("toc_max_level")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;
        markdown = generate_toc_for_markdown(&markdown, toc_max_level)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    }

    Ok(markdown)
}

async fn handle_browser_close(_args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    if browser::close().await {
        Ok("Browser session closed.".to_string())
    } else {
        Ok("No browser session was open.".to_string())
    }
}

fn obsidian_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, Box<dyn std::error::Error>> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter(key.to_string())) as Box<dyn std::error::Error>)
}

fn obsidian_result(value: Result<serde_json::Value, anyhow::Error>) -> Result<String, Box<dyn std::error::Error>> {
    let v = value.map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    Ok(serde_json::to_string_pretty(&v)?)
}

fn handle_obsidian_vault_index(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let include_orphans = args.get("include_orphans").and_then(|v| v.as_bool()).unwrap_or(false);
    obsidian_result(obsidian::tools::vault_index(Path::new(vault), include_orphans))
}

fn handle_obsidian_get_note(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let note = obsidian_arg(args, "note")?;
    let inline_embeds = args.get("inline_embeds").and_then(|v| v.as_bool()).unwrap_or(false);
    let depth = args.get("max_embed_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    obsidian_result(obsidian::tools::get_note(Path::new(vault), note, inline_embeds, depth))
}

fn handle_obsidian_resolve_link(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let link = obsidian_arg(args, "link")?;
    let from = args.get("from_note").and_then(|v| v.as_str());
    obsidian_result(obsidian::tools::resolve_link(Path::new(vault), link, from))
}

fn handle_obsidian_get_backlinks(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let note = obsidian_arg(args, "note")?;
    obsidian_result(obsidian::tools::get_backlinks(Path::new(vault), note))
}

fn handle_obsidian_search(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let query = obsidian_arg(args, "query")?;
    let mode = obsidian_arg(args, "mode")?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    obsidian_result(obsidian::tools::search(Path::new(vault), query, mode, limit))
}

fn handle_obsidian_list_tasks(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let status = args.get("status").and_then(|v| v.as_str());
    let note = args.get("note").and_then(|v| v.as_str());
    obsidian_result(obsidian::tools::list_tasks(Path::new(vault), status, note))
}

fn handle_obsidian_get_vault_config(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    obsidian_result(obsidian::tools::get_vault_config(Path::new(vault)))
}

fn handle_obsidian_create_note_from_template(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let title = args.get("title").and_then(|v| v.as_str());
    let template = args.get("template").and_then(|v| v.as_str());
    let daily = args.get("daily").and_then(|v| v.as_bool()).unwrap_or(false);
    obsidian_result(obsidian::tools::create_note_from_template(Path::new(vault), title, template, daily))
}

fn handle_obsidian_rename_note(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let note = obsidian_arg(args, "note")?;
    let new_name = obsidian_arg(args, "new_name")?;
    let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(true);
    obsidian_result(obsidian::tools::rename_note(Path::new(vault), note, new_name, dry_run))
}

fn handle_analyze_text(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let text = match (args.get("content").and_then(|v| v.as_str()), args.get("file_path").and_then(|v| v.as_str())) {
        (Some(c), _) => c.to_string(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(format!("Cannot read {}: {}", path, e))) as Box<dyn std::error::Error>)?,
        (None, None) => {
            return Err(Box::new(ConversionError::MissingParameter("content or file_path".to_string())));
        }
    };

    let provider = args.get("provider").and_then(|v| v.as_str()).unwrap_or("anthropic");
    let model = args.get("model").and_then(|v| v.as_str());
    let tokenizer_file = args.get("tokenizer_file").and_then(|v| v.as_str());
    let top = args.get("top").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let format = args.get("output_format").and_then(|v| v.as_str()).unwrap_or("markdown");

    let spec = textmetrics::TokenizerSpec::from_params(provider, model, tokenizer_file)
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    let metrics = textmetrics::analyze_text(&text, &spec)
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    if format == "json" {
        let mut v = serde_json::to_value(&metrics)?;
        if top != 0 {
            for key in ["word_freq", "char_freq", "token_freq"] {
                if let Some(arr) = v.get_mut(key).and_then(|a| a.as_array_mut()) {
                    arr.truncate(top);
                }
            }
        }
        Ok(serde_json::to_string_pretty(&v)?)
    } else {
        Ok(textmetrics::metrics_to_markdown(&metrics, top))
    }
}

fn handle_obsidian_convert_canvas(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let path = obsidian_arg(args, "canvas_path")?;
    obsidian_result(obsidian::tools::convert_canvas(Path::new(path)))
}

fn handle_obsidian_extract_dataview_fields(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let vault = obsidian_arg(args, "vault_path")?;
    let note = args.get("note").and_then(|v| v.as_str());
    let field = args.get("field").and_then(|v| v.as_str());
    obsidian_result(obsidian::tools::extract_dataview_fields(Path::new(vault), note, field))
}

fn handle_list_directory_files(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("directory".to_string())) as Box<dyn std::error::Error>)?;

    let recursive = args.get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let files = list_files_in_directory(directory, recursive)
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    let mut result = String::new();
    result.push_str(&format!("# Code Files in: {}\n\n", directory));
    result.push_str(&format!("Found {} files:\n\n", files.len()));

    for file in files {
        if let Some(path_str) = file.to_str() {
            result.push_str(&format!("- `{}`\n", path_str));
        }
    }

    Ok(result)
}

fn generate_toc_for_markdown(markdown: &str, max_level: usize) -> Result<String, Box<dyn std::error::Error>> {
    let headings = generate_toc(markdown, max_level)
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    if headings.is_empty() {
        return Ok(markdown.to_string());
    }

    let toc_content = format_toc(&headings, "Table of Contents");
    let result = toc_generator::insert_toc(markdown, &toc_content);

    Ok(result)
}

#[allow(clippy::too_many_arguments)] // mirrors the flat HTML-option tool schema
fn handle_html_file_conversion(
    file_path: &str,
    filename: &str,
    extract_metadata: bool,
    preserve_css_hints: bool,
    generate_toc_flag: bool,
    toc_max_level: usize,
    extract_images: bool,
    image_format: ImageFormat,
    convert_tables: bool,
    extract_forms: bool,
    preserve_comments: bool,
    extract_links: bool,
    analyze_headings: bool,
    extract_definition_lists: bool,
    extract_blockquotes: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    let extension = path.extension().and_then(|ext| ext.to_str());

    // For webarchive files, read as binary; for others, read as text
    let mut html_content = if extension == Some("webarchive") {
        let binary_content = std::fs::read(path)
            .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

        webarchive_parser::extract_html_from_webarchive(&binary_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?
    } else {
        // Read as text for HTML/HTM/MHTML files
        let content = std::fs::read_to_string(path)
            .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

        if extension == Some("mhtml") {
            extract_html_from_mhtml(&content)
                .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?
        } else {
            content
        }
    };

    // Extract blockquotes if needed
    let mut blockquote_summary = String::new();
    if extract_blockquotes {
        let blockquotes = blockquote_extractor::extract_blockquotes_from_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        if !blockquotes.is_empty() {
            blockquote_summary = blockquote_extractor::generate_blockquote_summary(&blockquotes);
        }
    }

    // Extract definition lists if needed
    let mut definition_list_summary = String::new();
    if extract_definition_lists {
        let lists = definition_list_converter::extract_definition_lists_from_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        if !lists.is_empty() {
            definition_list_summary = definition_list_converter::generate_definition_list_summary(&lists);
        }
    }

    // Analyze headings if needed
    let mut heading_summary = String::new();
    if analyze_headings {
        let headings = heading_analyzer::extract_headings_from_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        if !headings.is_empty() {
            let stats = heading_analyzer::analyze_heading_structure(&headings);
            heading_summary.push_str(&heading_analyzer::generate_heading_summary(&stats));
            heading_summary.push_str(&heading_analyzer::generate_heading_tree(&headings));
        }
    }

    // Extract links if needed
    let mut link_summary = String::new();
    if extract_links {
        let links = link_extractor::extract_links_from_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        if !links.is_empty() {
            link_summary = link_extractor::generate_link_summary(&links);
        }
    }

    // Process comments if needed
    let mut comment_summary = String::new();
    if preserve_comments {
        let (html, comments) = comment_extractor::process_comments_in_html(&html_content, true)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
        html_content = html;
        if !comments.is_empty() {
            comment_summary = comment_extractor::generate_comment_summary(&comments);
        }
    }

    // Extract forms if needed
    if extract_forms {
        html_content = form_extractor::process_forms_in_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?
            .0;
    }

    // Convert tables if needed
    if convert_tables {
        html_content = table_converter::convert_tables_in_html(&html_content)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    }

    // Process images if needed
    if extract_images {
        html_content = image_extractor::process_images_in_html(&html_content, image_format)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    }

    // Convert HTML to Markdown with optional metadata extraction and CSS hints
    let mut markdown = html_to_markdown_with_options(&html_content, extract_metadata, preserve_css_hints)
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    // Generate table of contents if requested
    if generate_toc_flag {
        markdown = generate_toc_for_markdown(&markdown, toc_max_level)
            .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;
    }

    // Add filename as heading if not empty (and metadata not extracted)
    let mut result = String::new();
    if !filename.is_empty() && !extract_metadata {
        result.push_str(&format!("# {}\n\n", filename));
    }

    // Add summaries if present (blockquotes, definition lists, headings, links, comments)
    if !blockquote_summary.is_empty() {
        result.push_str(&blockquote_summary);
        result.push('\n');
    }

    if !definition_list_summary.is_empty() {
        result.push_str(&definition_list_summary);
        result.push('\n');
    }

    if !heading_summary.is_empty() {
        result.push_str(&heading_summary);
        result.push('\n');
    }

    if !link_summary.is_empty() {
        result.push_str(&link_summary);
        result.push('\n');
    }

    if !comment_summary.is_empty() {
        result.push_str(&comment_summary);
        result.push('\n');
    }

    result.push_str(&markdown);

    Ok(result)
}

fn handle_get_file_summary(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let file_path = args.get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("file_path".to_string())) as Box<dyn std::error::Error>)?;

    let preview_length = args.get("preview_length")
        .and_then(|v| v.as_u64())
        .unwrap_or(300) as usize;

    let path = Path::new(file_path);
    let extension = path.extension().and_then(|ext| ext.to_str());

    // Binary/office documents: convert first, then summarize the resulting Markdown.
    if let Some(conv) = try_convert_binary_document(path) {
        let md = conv?;
        let mut summary = String::new();
        summary.push_str("## Headings\n\n");
        let heads: Vec<&str> = md.lines().filter(|l| l.trim_start().starts_with('#')).take(10).collect();
        if heads.is_empty() {
            summary.push_str("(No headings found)\n");
        } else {
            for h in &heads {
                summary.push_str(&format!("{}\n", h.trim()));
            }
        }
        summary.push_str("\n## Preview\n\n");
        let preview = if md.len() > preview_length {
            let end = (0..=preview_length).rev().find(|&i| md.is_char_boundary(i)).unwrap_or(0);
            format!("{}...", &md[..end])
        } else {
            md.clone()
        };
        summary.push_str(&format!("```\n{}\n```\n", preview));
        return Ok(summary);
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    let is_html = matches!(extension, Some("html" | "htm" | "mhtml" | "webarchive"));

    let mut summary = String::new();

    if is_html {
        // Extract metadata from meta tags
        summary.push_str("## Metadata\n\n");
        let document = scraper::Html::parse_document(&content);

        if let Ok(meta_sel) = scraper::Selector::parse("meta") {
            let mut meta_count = 0;
            for meta in document.select(&meta_sel) {
                if let Some(name) = meta.value().attr("name") {
                    if let Some(content_val) = meta.value().attr("content") {
                        summary.push_str(&format!("- **{}**: {}\n", name, content_val));
                        meta_count += 1;
                    }
                }
                if meta_count >= 5 {
                    break;
                }
            }

            if meta_count == 0 {
                summary.push_str("(No metadata found)\n");
            }
        }
        summary.push('\n');

        // Extract headings
        summary.push_str("## Headings\n\n");
        if let Ok(headings) = heading_analyzer::extract_headings_from_html(&content) {
            if headings.is_empty() {
                summary.push_str("(No headings found)\n");
            } else {
                for heading in headings.iter().take(10) {
                    let indent = "  ".repeat(heading.level - 1);
                    summary.push_str(&format!("{}H{}: {}\n", indent, heading.level, heading.text));
                }
                if headings.len() > 10 {
                    summary.push_str(&format!("... and {} more headings\n", headings.len() - 10));
                }
            }
        }
        summary.push('\n');
    }

    // Add content preview
    summary.push_str("## Preview\n\n");
    let preview_text = if is_html {
        // Strip HTML tags for preview - replace < and > with spaces
        let cleaned = content.replace(['<', '>'], " ");
        let text_only = cleaned
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join(" ");

        if text_only.len() > preview_length {
            format!("{}...", &text_only[..preview_length])
        } else {
            text_only
        }
    } else {
        // For non-HTML files, use raw content
        if content.len() > preview_length {
            format!("{}...", &content[..preview_length])
        } else {
            content.clone()
        }
    };

    summary.push_str(&format!("```\n{}\n```\n", preview_text));

    Ok(summary)
}

fn handle_batch_convert_files(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let file_paths = args.get("file_paths")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("file_paths".to_string())) as Box<dyn std::error::Error>)?;

    if file_paths.is_empty() {
        return Err(Box::new(ConversionError::ConversionFailed("file_paths array is empty".to_string())));
    }

    if file_paths.len() > 10 {
        return Err(Box::new(ConversionError::ConversionFailed("Maximum 10 files allowed".to_string())));
    }

    let extract_metadata = args.get("extract_metadata")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let convert_tables = args.get("convert_tables")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_images = args.get("extract_images")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let image_format_str = args.get("image_format")
        .and_then(|v| v.as_str())
        .unwrap_or("link");

    let image_format = ImageFormat::from_str(image_format_str)
        .unwrap_or(ImageFormat::Link);

    let extract_forms = args.get("extract_forms")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let extract_links = args.get("extract_links")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut result = String::new();

    for (idx, path_val) in file_paths.iter().enumerate() {
        if let Some(file_path) = path_val.as_str() {
            // Add separator between files
            if idx > 0 {
                result.push_str("\n---\n\n");
            }

            // Add filename header
            let path = Path::new(file_path);
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                result.push_str(&format!("# {}\n\n", filename));
            }

            // Binary/office documents: convert before the text read below.
            if let Some(conv) = try_convert_binary_document(path) {
                match conv {
                    Ok(md) => result.push_str(&md),
                    Err(e) => result.push_str(&format!("**Error converting file**: {}\n", e)),
                }
                continue;
            }

            // Convert file
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    result.push_str(&format!("**Error reading file**: {}\n", e));
                    continue;
                }
            };

            let extension = path.extension().and_then(|ext| ext.to_str());
            let is_html = matches!(extension, Some("html" | "htm" | "mhtml" | "webarchive"));

            let converted = if is_html && (extract_metadata || convert_tables || extract_forms || extract_images || extract_links) {
                let mut html_content = content.clone();

                if extract_links {
                    if let Ok(links) = link_extractor::extract_links_from_html(&html_content) {
                        if !links.is_empty() {
                            result.push_str(&link_extractor::generate_link_summary(&links));
                            result.push('\n');
                        }
                    }
                }

                if extract_forms {
                    if let Ok((html, _)) = form_extractor::process_forms_in_html(&html_content) {
                        html_content = html;
                    }
                }

                if convert_tables {
                    if let Ok(html) = table_converter::convert_tables_in_html(&html_content) {
                        html_content = html;
                    }
                }

                if extract_images {
                    if let Ok(html) = image_extractor::process_images_in_html(&html_content, image_format) {
                        html_content = html;
                    }
                }

                html_to_markdown_with_options(&html_content, extract_metadata, false)
                    .unwrap_or_else(|_| content.clone())
            } else {
                let language = detect_language(path);
                convert_to_markdown_with_options(&content, if language.is_empty() { None } else { Some(&language) }, None, false)
            };

            result.push_str(&converted);
        }
    }

    Ok(result)
}

fn handle_search_files(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("directory".to_string())) as Box<dyn std::error::Error>)?;

    let query = args.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("query".to_string())) as Box<dyn std::error::Error>)?;

    let max_results = args.get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;

    let context_chars = args.get("context_chars")
        .and_then(|v| v.as_u64())
        .unwrap_or(150) as usize;

    let query_lower = query.to_lowercase();
    let query_str = query.to_string();
    let mut matches = Vec::new();
    let mut files_checked = 0;

    // Walk directory recursively
    fn search_dir(
        dir: &Path,
        query_lower: &str,
        query_str: &str,
        context_chars: usize,
        matches: &mut Vec<(String, String)>,
        files_checked: &mut usize,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let _ = search_dir(&path, query_lower, query_str, context_chars, matches, files_checked);
            } else {
                *files_checked += 1;
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let content_lower = content.to_lowercase();
                    if let Some(pos) = content_lower.find(query_lower) {
                        let start = pos.saturating_sub(context_chars);
                        let end = (pos + query_lower.len() + context_chars).min(content.len());
                        let snippet = &content[start..end];

                        if let Some(path_str) = path.to_str() {
                            let highlight_snippet = if start > 0 {
                                format!("...{}...", snippet.replace(query_str, &format!("**{}**", query_str)))
                            } else {
                                snippet.replace(query_str, &format!("**{}**", query_str))
                            };
                            matches.push((path_str.to_string(), highlight_snippet));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    let path = Path::new(directory);
    let _ = search_dir(path, &query_lower, &query_str, context_chars, &mut matches, &mut files_checked);

    let mut result = String::new();
    result.push_str(&format!("# Search Results for: {}\n\n", query));
    result.push_str(&format!("**Query:** `{}`  \n", query));
    result.push_str(&format!("**Matches:** {} files (checked {} files)\n\n", matches.len().min(max_results), files_checked));

    if matches.is_empty() {
        result.push_str("No matches found.\n");
    } else {
        for (file_path, snippet) in matches.iter().take(max_results) {
            result.push_str(&format!("## {}\n\n", file_path));
            result.push_str(&format!("```\n{}\n```\n\n", snippet));
        }

        if matches.len() > max_results {
            result.push_str(&format!("\n... and {} more matches (limited to {})\n", matches.len() - max_results, max_results));
        }
    }

    Ok(result)
}

fn handle_get_recently_modified_files(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("directory".to_string())) as Box<dyn std::error::Error>)?;

    let limit = args.get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;

    let recursive = args.get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut files_info = Vec::new();

    // Walk directory
    fn collect_files(
        dir: &Path,
        recursive: bool,
        files_info: &mut Vec<(String, u64, u64)>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() && recursive {
                let _ = collect_files(&path, recursive, files_info);
            } else if path.is_file() {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.elapsed() {
                            let secs_ago = duration.as_secs();
                            let size = metadata.len();
                            if let Some(path_str) = path.to_str() {
                                files_info.push((path_str.to_string(), secs_ago, size));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    let path = Path::new(directory);
    let _ = collect_files(path, recursive, &mut files_info);

    // Sort by modification time (most recent first)
    files_info.sort_by_key(|info| info.1);

    let mut result = String::new();
    result.push_str(&format!("# Recently Modified Files in: {}\n\n", directory));

    result.push_str("| File | Modified | Size |\n");
    result.push_str("|------|----------|------|\n");

    for (file_path, secs_ago, size) in files_info.iter().take(limit) {
        let time_str = if *secs_ago < 60 {
            format!("{} sec ago", secs_ago)
        } else if *secs_ago < 3600 {
            format!("{} min ago", secs_ago / 60)
        } else if *secs_ago < 86400 {
            format!("{} hrs ago", secs_ago / 3600)
        } else {
            format!("{} days ago", secs_ago / 86400)
        };

        let size_str = if *size < 1024 {
            format!("{} B", size)
        } else if *size < 1024 * 1024 {
            format!("{:.1} KB", *size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", *size as f64 / (1024.0 * 1024.0))
        };

        result.push_str(&format!("| `{}` | {} | {} |\n", file_path, time_str, size_str));
    }

    if files_info.len() > limit {
        result.push_str(&format!("\n**Total:** {} files (showing {} most recent)\n", files_info.len(), limit));
    } else {
        result.push_str(&format!("\n**Total:** {} files\n", files_info.len()));
    }

    Ok(result)
}

fn handle_get_vault_statistics(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("directory".to_string())) as Box<dyn std::error::Error>)?;

    use std::collections::HashMap;

    let mut ext_counts: HashMap<String, usize> = HashMap::new();
    let mut top_level_folders = std::collections::BTreeSet::new();
    let mut tag_counts: HashMap<String, usize> = HashMap::new();

    fn collect_stats(
        dir: &Path,
        base_depth: usize,
        ext_counts: &mut HashMap<String, usize>,
        top_level_folders: &mut std::collections::BTreeSet<String>,
        tag_counts: &mut HashMap<String, usize>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let depth = path.iter().count();
                if depth == base_depth + 1 {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !name.starts_with('.') {
                            top_level_folders.insert(name.to_string());
                        }
                    }
                }
                if depth < base_depth + 3 {
                    let _ = collect_stats(&path, base_depth, ext_counts, top_level_folders, tag_counts);
                }
            } else {
                let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !file_ext.is_empty() {
                    *ext_counts.entry(file_ext.to_string()).or_insert(0) += 1;
                }

                if file_ext == "md" || file_ext == "txt" {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        for line in content.lines() {
                            if line.starts_with("tags:") {
                                let tags_str = line.strip_prefix("tags:").unwrap_or("").trim();
                                if tags_str.starts_with('[') && tags_str.ends_with(']') {
                                    let csv = tags_str.trim_start_matches('[').trim_end_matches(']');
                                    for tag in csv.split(',') {
                                        let clean_tag = tag.trim().trim_matches('"').trim_matches('\'');
                                        if !clean_tag.is_empty() {
                                            *tag_counts.entry(clean_tag.to_string()).or_insert(0) += 1;
                                        }
                                    }
                                }
                            } else if line.trim().starts_with("#") && !line.starts_with("##") {
                                continue;
                            } else {
                                for word in line.split_whitespace() {
                                    if word.starts_with('#') && word.len() > 1 {
                                        let tag = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-');
                                        if !tag.is_empty() && tag.chars().next().is_some_and(|c| c.is_alphabetic()) {
                                            *tag_counts.entry(tag.to_string()).or_insert(0) += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    let base = Path::new(directory);
    let base_depth = base.iter().count();
    let _ = collect_stats(base, base_depth, &mut ext_counts, &mut top_level_folders, &mut tag_counts);

    let mut result = String::new();
    result.push_str("# Vault Statistics\n\n");

    result.push_str("## File Distribution\n\n");
    result.push_str("| Extension | Count |\n");
    result.push_str("|-----------|-------|\n");
    let mut exts: Vec<_> = ext_counts.iter().collect();
    exts.sort_by_key(|&(_, count)| std::cmp::Reverse(*count));
    for (ext, count) in exts {
        result.push_str(&format!("| .{} | {} |\n", ext, count));
    }

    result.push_str("\n## Top-Level Folders\n\n");
    for folder in &top_level_folders {
        result.push_str(&format!("- {}\n", folder));
    }

    result.push_str("\n## Top 20 Tags\n\n");
    let mut tags: Vec<_> = tag_counts.iter().collect();
    tags.sort_by_key(|&(_, count)| std::cmp::Reverse(*count));
    for (tag, count) in tags.iter().take(20) {
        result.push_str(&format!("- **#{}** ({} occurrences)\n", tag, count));
    }

    Ok(result)
}

fn handle_extract_active_todos(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let folder_scope = args.get("folder_scope")
        .and_then(|v| v.as_str());

    let scan_dir = folder_scope.unwrap_or(directory);

    let mut todos_by_file: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    let mut total_todos = 0;

    fn scan_for_todos(
        dir: &Path,
        todos_by_file: &mut std::collections::BTreeMap<String, Vec<String>>,
        total: &mut usize,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let _ = scan_for_todos(&path, todos_by_file, total);
            } else {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        if line.contains("- [ ]") {
                            let path_str = path.to_string_lossy().to_string();
                            todos_by_file.entry(path_str).or_default().push(line.trim().to_string());
                            *total += 1;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    let path = Path::new(scan_dir);
    let _ = scan_for_todos(path, &mut todos_by_file, &mut total_todos);

    let mut result = String::new();
    result.push_str("# Active Tasks\n\n");

    if todos_by_file.is_empty() {
        result.push_str("No active tasks found.\n");
    } else {
        for (file_path, tasks) in todos_by_file.iter() {
            result.push_str(&format!("## {}\n\n", file_path));
            for task in tasks {
                result.push_str(&format!("{}\n", task));
            }
            result.push('\n');
        }

        result.push_str(&format!("**Total:** {} tasks across {} files\n", total_todos, todos_by_file.len()));
    }

    Ok(result)
}

fn handle_safe_append_or_replace_section(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("path".to_string())) as Box<dyn std::error::Error>)?;

    let heading = args.get("heading")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("heading".to_string())) as Box<dyn std::error::Error>)?;

    let content = args.get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("content".to_string())) as Box<dyn std::error::Error>)?;

    let mode = args.get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("append");

    let file_path = Path::new(path);
    let current_content = std::fs::read_to_string(file_path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    let lines: Vec<&str> = current_content.lines().collect();
    let mut heading_line_idx = None;
    let mut heading_level = 0;

    for (idx, line) in lines.iter().enumerate() {
        if let Some(stripped) = line.strip_prefix('#') {
            let level = stripped.len() - stripped.trim_start_matches('#').len();
            if level > 0 && level <= 6 {
                let heading_text = stripped.trim_start_matches('#').trim();
                if heading_text == heading {
                    heading_line_idx = Some(idx);
                    heading_level = level;
                    break;
                }
            }
        }
    }

    if heading_line_idx.is_none() {
        return Err(Box::new(ConversionError::ConversionFailed(format!("Heading '{}' not found", heading))));
    }

    let start_idx = heading_line_idx.unwrap() + 1;
    let mut end_idx = lines.len();

    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        if let Some(stripped) = line.strip_prefix('#') {
            let level = stripped.len() - stripped.trim_start_matches('#').len();
            if level > 0 && level <= heading_level {
                end_idx = idx;
                break;
            }
        }
    }

    let mut result_lines = Vec::new();
    result_lines.extend_from_slice(&lines[..start_idx]);

    if mode == "overwrite" {
        result_lines.push(content);
    } else {
        result_lines.extend_from_slice(&lines[start_idx..end_idx]);
        result_lines.push(content);
    }

    result_lines.extend_from_slice(&lines[end_idx..]);

    let updated_content = result_lines.join("\n");
    std::fs::write(file_path, updated_content)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    Ok(format!("Updated section '{}' in {} (mode: {})", heading, path, mode))
}

fn handle_resolve_and_validate_links(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let links_array = args.get("links")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("links".to_string())) as Box<dyn std::error::Error>)?;

    let directory = args.get("directory")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let mut all_files = Vec::new();

    fn collect_all_files(dir: &Path, files: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let _ = collect_all_files(&path, files);
            } else {
                if let Some(path_str) = path.to_str() {
                    files.push(path_str.to_lowercase());
                }
            }
        }
        Ok(())
    }

    let base = Path::new(directory);
    let _ = collect_all_files(base, &mut all_files);

    let mut result = String::new();
    result.push_str("# Link Validation Results\n\n");

    for link_val in links_array {
        if let Some(link) = link_val.as_str() {
            let link_lower = link.to_lowercase();
            let mut found = false;
            let mut closest_match = None;

            for file in &all_files {
                if file.ends_with(&link_lower) || file.contains(&link_lower) {
                    found = true;
                    closest_match = Some(file.clone());
                    break;
                }
            }

            if found {
                result.push_str(&format!("✅ **{}** — Found: {}\n", link, closest_match.unwrap_or_default()));
            } else {
                let mut fuzzy_match = None;
                for file in &all_files {
                    if file.contains(&link_lower) {
                        fuzzy_match = Some(file.clone());
                        break;
                    }
                }

                if let Some(fm) = fuzzy_match {
                    result.push_str(&format!("⚠️ **{}** — Partial match: {}\n", link, fm));
                } else {
                    result.push_str(&format!("❌ **{}** — Not found\n", link));
                }
            }
        }
    }

    Ok(result)
}

fn handle_upsert_markdown_table(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("path".to_string())) as Box<dyn std::error::Error>)?;

    let heading = args.get("heading")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("heading".to_string())) as Box<dyn std::error::Error>)?;

    let headers = args.get("headers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("headers".to_string())) as Box<dyn std::error::Error>)?;

    let rows = args.get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("rows".to_string())) as Box<dyn std::error::Error>)?;

    let mut header_strs = Vec::new();
    for h in headers {
        if let Some(hs) = h.as_str() {
            header_strs.push(hs.replace('|', "\\|"));
        }
    }

    let mut row_vecs = Vec::new();
    for row in rows {
        if let Some(row_arr) = row.as_array() {
            let mut row_cells = Vec::new();
            for cell in row_arr {
                if let Some(cs) = cell.as_str() {
                    row_cells.push(cs.replace('|', "\\|"));
                }
            }
            if !row_cells.is_empty() {
                row_vecs.push(row_cells);
            }
        }
    }

    let row_count = row_vecs.len();
    let col_count = header_strs.len();

    let mut table = String::new();
    table.push_str("| ");
    table.push_str(&header_strs.join(" | "));
    table.push_str(" |\n");
    table.push('|');
    for _ in 0..col_count {
        table.push_str("---|");
    }
    table.push('\n');

    for row_cells in row_vecs {
        table.push_str("| ");
        table.push_str(&row_cells.join(" | "));
        table.push_str(" |\n");
    }

    let file_path = Path::new(path);
    let success_msg = format!("Table inserted/updated under '{}' in {} ({} rows, {} columns)", heading, path, row_count, col_count);

    if file_path.exists() {
        let current = std::fs::read_to_string(file_path)
            .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

        let lines: Vec<&str> = current.lines().collect();
        let mut heading_idx = None;

        for (idx, line) in lines.iter().enumerate() {
            if let Some(stripped) = line.strip_prefix('#') {
                let heading_text = stripped.trim_start_matches('#').trim();
                if heading_text == heading {
                    heading_idx = Some(idx);
                    break;
                }
            }
        }

        if let Some(h_idx) = heading_idx {
            for idx in (h_idx + 1)..lines.len() {
                if lines[idx].starts_with('|') {
                    let table_start = idx;
                    let mut table_end = idx + 1;
                    while table_end < lines.len() && lines[table_end].starts_with('|') {
                        table_end += 1;
                    }

                    let mut new_lines = lines[..table_start].to_vec();
                    new_lines.push(&table);
                    new_lines.extend_from_slice(&lines[table_end..]);

                    std::fs::write(file_path, new_lines.join("\n"))
                        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
                    return Ok(success_msg);
                }
                if let Some(next_h) = lines[idx].strip_prefix('#') {
                    let level = next_h.len() - next_h.trim_start_matches('#').len();
                    if level > 0 {
                        break;
                    }
                }
            }

            // No existing table under the heading: insert one after it.
            let mut new_lines = lines.clone();
            new_lines.insert(h_idx + 1, "");
            new_lines.insert(h_idx + 2, &table);
            std::fs::write(file_path, new_lines.join("\n"))
                .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
            return Ok(success_msg);
        } else {
            let mut new_content = current.clone();
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
            new_content.push_str(&format!("\n## {}\n\n{}", heading, table));
            std::fs::write(file_path, new_content)
                .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
            return Ok(success_msg);
        }
    }

    let new_content = format!("## {}\n\n{}", heading, table);
    std::fs::write(file_path, new_content)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    Ok(success_msg)
}

fn handle_read_file(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("path".to_string())) as Box<dyn std::error::Error>)?;

    let start_line = args.get("start_line")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as usize;

    let end_line = args.get("end_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    let lines: Vec<&str> = content.lines().collect();
    let start_idx = (start_line.saturating_sub(1)).min(lines.len());
    let end_idx = end_line.unwrap_or(lines.len()).min(lines.len());

    let selected_lines = if start_idx < end_idx && start_idx < lines.len() {
        lines[start_idx..end_idx].join("\n")
    } else {
        String::new()
    };

    let file_ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("txt");
    let output = format!("```{}\n{}\n```", file_ext, selected_lines);

    Ok(output)
}

fn handle_create_or_append_file(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("path".to_string())) as Box<dyn std::error::Error>)?;

    let content = args.get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("content".to_string())) as Box<dyn std::error::Error>)?;

    let mode = args.get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("create");

    let file_path = Path::new(path);

    // Create parent directories if needed
    if let Some(parent) = file_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
        }
    }

    match mode {
        "create" => {
            if file_path.exists() {
                return Err(Box::new(ConversionError::ConversionFailed(format!("File already exists: {}", path))));
            }
            std::fs::write(file_path, content)
                .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
        }
        "append" => {
            let mut existing = if file_path.exists() {
                std::fs::read_to_string(file_path)
                    .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?
            } else {
                String::new()
            };
            existing.push_str(content);
            std::fs::write(file_path, existing)
                .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
        }
        "overwrite" => {
            std::fs::write(file_path, content)
                .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
        }
        _ => {
            return Err(Box::new(ConversionError::ConversionFailed(format!("Invalid mode: {}", mode))));
        }
    }

    Ok(format!("File {} with mode '{}'", if file_path.exists() { "created/updated" } else { "processed" }, mode))
}

/// Render detailed help for a tool from its tools/list schema definition:
/// description plus a parameter table. Returns None for unknown tool names.
fn schema_help(tool_name: &str) -> Option<String> {
    let defs = tool_definitions();
    let tool = defs.as_array()?.iter().find(|t| t["name"] == tool_name)?;
    let mut help = String::new();
    if let Some(desc) = tool["description"].as_str() {
        help.push_str(desc);
        help.push_str("\n\n");
    }
    let schema = &tool["inputSchema"];
    let required: Vec<&str> = schema["required"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if let Some(props) = schema["properties"].as_object() {
        if !props.is_empty() {
            help.push_str("**Parameters:**\n");
            help.push_str("| Name | Type | Required | Description |\n");
            help.push_str("|------|------|----------|-------------|\n");
            for (name, prop) in props {
                help.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    name,
                    prop["type"].as_str().unwrap_or("-"),
                    if required.contains(&name.as_str()) { "yes" } else { "no" },
                    prop["description"].as_str().unwrap_or("-").replace('|', "\\|"),
                ));
            }
        } else {
            help.push_str("No parameters.\n");
        }
    }
    Some(help)
}

fn handle_get_tool_help(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let tool_name = args.get("tool_name")
        .and_then(|v| v.as_str());

    let mut help = String::new();

    if let Some(name) = tool_name {
        // Detailed help for specific tool
        help.push_str(&format!("# Tool: {}\n\n", name));

        match name {
            "read_file" => {
                help.push_str("Read raw file content without conversion.\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Default | Description |\n");
                help.push_str("|------|------|----------|---------|-------------|\n");
                help.push_str("| path | string | yes | - | File path to read |\n");
                help.push_str("| start_line | integer | no | 1 | First line to return |\n");
                help.push_str("| end_line | integer | no | - | Last line to return (omit for full file) |\n\n");
                help.push_str("**Example:**\n```json\n{\"path\": \"config.json\", \"start_line\": 1, \"end_line\": 50}\n```\n");
            }
            "create_or_append_file" => {
                help.push_str("Create a new file or append/overwrite an existing file.\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Default | Description |\n");
                help.push_str("|------|------|----------|---------|-------------|\n");
                help.push_str("| path | string | yes | - | File path |\n");
                help.push_str("| content | string | yes | - | Content to write |\n");
                help.push_str("| mode | string | no | create | 'create', 'append', or 'overwrite' |\n\n");
                help.push_str("**Example:**\n```json\n{\"path\": \"notes.md\", \"content\": \"# Hello\", \"mode\": \"create\"}\n```\n");
            }
            "get_tool_help" => {
                help.push_str("Get help on available tools.\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Default | Description |\n");
                help.push_str("|------|------|----------|---------|-------------|\n");
                help.push_str("| tool_name | string | no | - | Name of a specific tool (omit for full list) |\n\n");
                help.push_str("**Example:**\n```json\n{\"tool_name\": \"read_file\"}\n```\n");
            }
            "move_or_rename_file" => {
                help.push_str("Move or rename a file or directory.\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Description |\n");
                help.push_str("|------|------|----------|-------------|\n");
                help.push_str("| old_path | string | yes | Current path |\n");
                help.push_str("| new_path | string | yes | Destination path |\n\n");
                help.push_str("**Example:**\n```json\n{\"old_path\": \"notes/old.md\", \"new_path\": \"archive/new.md\"}\n```\n");
            }
            "delete_file" => {
                help.push_str("Permanently delete a file or directory (irreversible).\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Default | Description |\n");
                help.push_str("|------|------|----------|---------|-------------|\n");
                help.push_str("| path | string | yes | - | File or directory to delete |\n");
                help.push_str("| recursive | bool | no | false | Required true for non-empty directories |\n\n");
                help.push_str("**Example:**\n```json\n{\"path\": \"temp_file.txt\"}\n```\n");
            }
            "batch_create_notes" => {
                help.push_str("Create multiple files in one call.\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Description |\n");
                help.push_str("|------|------|----------|-------------|\n");
                help.push_str("| notes | array | yes | Array of {path, content} objects (up to 20) |\n\n");
                help.push_str("**Example:**\n```json\n{\"notes\": [{\"path\": \"note1.md\", \"content\": \"# Title\"}, {\"path\": \"note2.md\", \"content\": \"# Note\"}]}\n```\n");
            }
            "update_note_properties" => {
                help.push_str("Update YAML frontmatter properties without touching the file body.\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Description |\n");
                help.push_str("|------|------|----------|-------------|\n");
                help.push_str("| path | string | yes | File path |\n");
                help.push_str("| properties | object | yes | Key-value pairs (set to null to remove) |\n\n");
                help.push_str("**Example:**\n```json\n{\"path\": \"note.md\", \"properties\": {\"status\": \"In Progress\", \"tags\": [\"ai\", \"test\"]}}\n```\n");
            }
            "find_note_by_alias_or_title" => {
                help.push_str("Fuzzy lookup to find a file by name, filename, or alias.\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Default | Description |\n");
                help.push_str("|------|------|----------|---------|-------------|\n");
                help.push_str("| search_term | string | yes | - | Name or snippet to search |\n");
                help.push_str("| directory | string | no | . | Root directory |\n\n");
                help.push_str("**Example:**\n```json\n{\"search_term\": \"Project Alpha\", \"directory\": \".\"}\n```\n");
            }
            "get_graph_relationships" => {
                help.push_str("Map wiki-link connections for a file.\n\n");
                help.push_str("**Parameters:**\n");
                help.push_str("| Name | Type | Required | Default | Description |\n");
                help.push_str("|------|------|----------|---------|-------------|\n");
                help.push_str("| path | string | yes | - | File to analyze |\n");
                help.push_str("| directory | string | no | . | Root directory to scan |\n\n");
                help.push_str("**Example:**\n```json\n{\"path\": \"README.md\", \"directory\": \".\"}\n```\n");
            }
            // Any tool without a hand-written help block above: render help
            // straight from its tools/list schema definition.
            other => match schema_help(other) {
                Some(text) => help.push_str(&text),
                None => help.push_str("Unknown tool. Use get_tool_help without tool_name to see all tools.\n"),
            },
        }
    } else {
        // Summary of all tools
        help.push_str("# Available Tools\n\n");
        help.push_str("**62 tools** across format conversion, browser-based web capture, file/vault operations, Obsidian vault support, an AI/RAG toolkit, and optional Claude-backed generation.\n\n");
        help.push_str("| Tool | Description |\n");
        help.push_str("|------|-------------|\n");
        help.push_str("| convert_file | Convert file to Markdown with HTML processing options |\n");
        help.push_str("| convert_text | Convert raw text to Markdown |\n");
        help.push_str("| convert_from_source | Convert from file, URL, or stdin to Markdown |\n");
        help.push_str("| list_directory_files | List code files in a directory |\n");
        help.push_str("| get_file_summary | Lightweight file snapshot (metadata, headings, preview) |\n");
        help.push_str("| batch_convert_files | Convert multiple files (up to 10) in one call |\n");
        help.push_str("| search_files | Search files with context snippets |\n");
        help.push_str("| get_recently_modified_files | List recently modified files with timestamps |\n");
        help.push_str("| get_vault_statistics | Global vault statistics (files, folders, tags) |\n");
        help.push_str("| extract_active_todos | Find all unchecked tasks across files |\n");
        help.push_str("| safe_append_or_replace_section | Update content under a heading |\n");
        help.push_str("| resolve_and_validate_links | Validate file links |\n");
        help.push_str("| upsert_markdown_table | Insert/overwrite Markdown table under heading |\n");
        help.push_str("| read_file | Read raw file content without conversion |\n");
        help.push_str("| create_or_append_file | Create or modify files |\n");
        help.push_str("| get_tool_help | Get help on tools (you are here) |\n");
        help.push_str("| move_or_rename_file | Move or rename files |\n");
        help.push_str("| delete_file | Delete files or directories |\n");
        help.push_str("| batch_create_notes | Create multiple files |\n");
        help.push_str("| update_note_properties | Update YAML frontmatter |\n");
        help.push_str("| find_note_by_alias_or_title | Fuzzy file lookup |\n");
        help.push_str("| get_graph_relationships | Map wiki-link connections |\n");
        help.push_str("| chunk_markdown | Heading-aware, token-bounded chunks for RAG |\n");
        help.push_str("| extract_chunks_for_rag | Convert any file then chunk to JSON for embeddings |\n");
        help.push_str("| get_document_outline | Nested heading outline (Markdown or JSON tree) |\n");
        help.push_str("| search_content | Ranked search inside converted document content |\n");
        help.push_str("| get_text_statistics | Word counts, distinct words, per-word frequency |\n");
        help.push_str("| get_corpus_statistics | Per-document + corpus-wide word statistics |\n");
        help.push_str("| extract_tags | Tag index (#tags + frontmatter) for a note or vault |\n");
        help.push_str("| extract_keywords | Salient terms via TF-IDF |\n");
        help.push_str("| find_related_notes | Similar notes via cosine similarity |\n");
        help.push_str("| summarize_document | Extractive TL;DR (no LLM) |\n");
        help.push_str("| extract_qa_pairs | Mine Q/A pairs for flashcards / RAG ground-truth |\n");
        help.push_str("| extract_entities | URLs, emails, dates, names with counts |\n");
        help.push_str("| build_knowledge_index | One JSON artifact: summary+outline+tags+keywords+chunks |\n");
        help.push_str("| retrieve_context | RAG retrieval: budgeted context block + citations for a query |\n");
        help.push_str("| count_tokens | Estimate tokens and model context-window fit |\n");
        help.push_str("| find_duplicates | Near-duplicate detection (SimHash) across a directory |\n");
        help.push_str("| cluster_documents | Topic clustering by cosine similarity |\n");
        help.push_str("| analyze_readability | Flesch reading ease / grade level |\n");
        help.push_str("| detect_natural_language | Detect natural language (EN/ES/FR/DE/PT/IT) |\n");
        help.push_str("| classify_document | Heuristic topic/content-type classification |\n");
        help.push_str("| ai_summarize | Abstractive summary via Claude API (needs ANTHROPIC_API_KEY) |\n");
        help.push_str("| ai_ask | RAG Q&A via Claude API with citations |\n");
        help.push_str("| ai_tag | Suggest tags via Claude API |\n");
        help.push_str("| ai_translate | Translate via Claude API |\n");
        help.push_str("| ai_classify | Classify into given labels via Claude API |\n");
        help.push_str("| browser_open_url | Open a URL in Chromium (headless or visible for CAPTCHA/login) |\n");
        help.push_str("| browser_capture_markdown | Convert the rendered page in the browser session to Markdown |\n");
        help.push_str("| browser_close | Close the Chromium browser session |\n");
        help.push_str("| obsidian_vault_index | Vault summary: tags, aliases, broken/ambiguous links, orphans |\n");
        help.push_str("| obsidian_get_note | Note with parsed frontmatter, links, tags; optional embed transclusion |\n");
        help.push_str("| obsidian_resolve_link | Resolve [[wikilink]] (alias/#heading/#^block) shortest-path style |\n");
        help.push_str("| obsidian_get_backlinks | Inbound links in all forms with context |\n");
        help.push_str("| obsidian_search | Search by tag, alias, frontmatter field, or text |\n");
        help.push_str("| obsidian_list_tasks | Checkbox tasks (all states) with Tasks-plugin dates |\n");
        help.push_str("| obsidian_get_vault_config | .obsidian settings: daily notes, templates, attachments |\n");
        help.push_str("| obsidian_create_note_from_template | New note or daily note with {{date}}/{{title}} |\n");
        help.push_str("| obsidian_rename_note | Rename note + rewrite inbound links (dry-run default) |\n");
        help.push_str("| obsidian_convert_canvas | .canvas file to structured Markdown |\n");
        help.push_str("| obsidian_extract_dataview_fields | Inline key:: value + frontmatter fields |\n");
        help.push_str("| analyze_text | Words/chars/spaces/tokens + word, character & token frequency tables, provider-aware tokenizers |\n\n");
        help.push_str("Run the binary as `to_markdown_mcp tui [PATH]` for an interactive terminal Markdown/vault viewer.\n\n");
        help.push_str("Start the server with `--base-dir DIR` (repeatable or comma-separated for multiple vaults) to set default directories: relative file/vault paths resolve against them (first existing match wins, new files go to the first dir), and vault_path/directory parameters may be omitted entirely.\n\n");
        help.push_str("Supported input formats: text/code, HTML/HTM/MHTML/webarchive, PDF, DOCX, DOC, RTF, ODT, XLSX/XLS/ODS/CSV, PPTX/ODP, EML, EPUB, MOBI, RSS/Atom, and markup (wiki, rst, adoc, org, tex, textile).\n\n");
        help.push_str("RAG/knowledge tools accept `output_format: \"json\"` for machine-readable output.\n\n");
        help.push_str("Use `get_tool_help` with a tool_name parameter for detailed help on a specific tool.\n");
    }

    Ok(help)
}

fn handle_move_or_rename_file(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let old_path = args.get("old_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("old_path".to_string())) as Box<dyn std::error::Error>)?;

    let new_path = args.get("new_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("new_path".to_string())) as Box<dyn std::error::Error>)?;

    let new_path_obj = Path::new(new_path);
    if let Some(parent) = new_path_obj.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
        }
    }

    std::fs::rename(old_path, new_path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    Ok(format!("✓ Moved '{}' to '{}'", old_path, new_path))
}

fn handle_delete_file(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("path".to_string())) as Box<dyn std::error::Error>)?;

    let recursive = args.get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let path_obj = Path::new(path);

    if path_obj.is_dir() {
        if !recursive
            && path_obj.read_dir()
                .map(|mut d| d.next().is_some())
                .unwrap_or(false)
            {
                return Err("Directory is not empty. Set recursive=true to delete non-empty directories".into());
            }
        std::fs::remove_dir_all(path)
            .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
        Ok(format!("✓ Deleted directory '{}'", path))
    } else {
        std::fs::remove_file(path)
            .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
        Ok(format!("✓ Deleted file '{}'", path))
    }
}

fn handle_batch_create_notes(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let notes = args.get("notes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("notes".to_string())) as Box<dyn std::error::Error>)?;

    if notes.is_empty() {
        return Err("notes array must not be empty".into());
    }

    if notes.len() > 20 {
        return Err("notes array can contain at most 20 items".into());
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();

    for (idx, note) in notes.iter().enumerate() {
        let path = note.get("path")
            .and_then(|v| v.as_str());
        let content = note.get("content")
            .and_then(|v| v.as_str());

        match (path, content) {
            (Some(p), Some(c)) => {
                let path_obj = Path::new(p);
                if let Some(parent) = path_obj.parent() {
                    if !parent.as_os_str().is_empty() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            errors.push(format!("Note {}: Failed to create parent dirs for '{}': {}", idx + 1, p, e));
                            continue;
                        }
                    }
                }

                match std::fs::write(p, c) {
                    Ok(_) => results.push(format!("Created '{}'", p)),
                    Err(e) => errors.push(format!("Note {}: Failed to create '{}': {}", idx + 1, p, e)),
                }
            }
            _ => errors.push(format!("Note {}: Missing 'path' or 'content'", idx + 1)),
        }
    }

    let mut output = format!("✓ Created {} file(s)", results.len());
    for r in results {
        output.push('\n');
        output.push_str(&r);
    }

    if !errors.is_empty() {
        output.push_str("\n\n❌ Errors:\n");
        for e in errors {
            output.push('\n');
            output.push_str(&e);
        }
    }

    Ok(output)
}

fn handle_update_note_properties(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("path".to_string())) as Box<dyn std::error::Error>)?;

    let properties = args.get("properties")
        .and_then(|v| v.as_object())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("properties".to_string())) as Box<dyn std::error::Error>)?;

    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    let lines: Vec<&str> = content.lines().collect();
    let (_yaml_start, yaml_end) = if !lines.is_empty() && lines[0] == "---" {
        let mut end = 0;
        for (i, line) in lines.iter().enumerate().skip(1) {
            if *line == "---" {
                end = i;
                break;
            }
        }
        if end > 0 {
            (0, Some(end))
        } else {
            (0, None)
        }
    } else {
        (0, None)
    };

    let mut yaml_lines = Vec::new();
    if let Some(end) = yaml_end {
        for line in &lines[1..end] {
            yaml_lines.push(line.to_string());
        }
    }

    for (key, value) in properties.iter() {
        if value.is_null() {
            yaml_lines.retain(|line| !line.starts_with(&format!("{}: ", key)) && !line.starts_with(&format!("{}:", key)));
        } else {
            let value_str = if value.is_string() {
                value.as_str().unwrap().to_string()
            } else {
                serde_json::to_string(value).unwrap_or_default()
            };

            let found = yaml_lines.iter_mut().any(|line| {
                if line.starts_with(&format!("{}: ", key)) || line.starts_with(&format!("{}:", key)) {
                    *line = format!("{}: {}", key, value_str);
                    true
                } else {
                    false
                }
            });

            if !found {
                yaml_lines.push(format!("{}: {}", key, value_str));
            }
        }
    }

    let mut new_content = String::new();
    new_content.push_str("---\n");
    for line in yaml_lines {
        new_content.push_str(&line);
        new_content.push('\n');
    }
    new_content.push_str("---\n");

    if let Some(end) = yaml_end {
        for line in &lines[end + 1..] {
            new_content.push_str(line);
            new_content.push('\n');
        }
    } else if !lines.is_empty() {
        for line in lines {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }

    std::fs::write(path, new_content)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    Ok(format!("✓ Updated properties in '{}'", path))
}

fn handle_find_note_by_alias_or_title(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let search_term = args.get("search_term")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("search_term".to_string())) as Box<dyn std::error::Error>)?;

    let directory = args.get("directory")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let search_lower = search_term.to_lowercase();
    let mut results = Vec::new();

    fn walk_dir(path: &Path, search_lower: &str, results: &mut Vec<(i32, String)>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let _ = walk_dir(&path, search_lower, results);
            } else if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                let mut score = 0;
                let file_name_lower = file_name.to_lowercase();

                if file_name_lower == search_lower {
                    score = 30;
                } else if file_name_lower.contains(search_lower) {
                    score = 20;
                } else if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.to_lowercase().contains(search_lower) {
                        if let Some(start) = content.to_lowercase().find("title:") {
                            let title_section = &content[start..];
                            if title_section.contains(search_lower) {
                                score = 15;
                            } else {
                                score = 5;
                            }
                        }
                    }
                }

                if score > 0 {
                    results.push((score, path.to_string_lossy().to_string()));
                }
            }
        }
        Ok(())
    }

    walk_dir(Path::new(directory), &search_lower, &mut results)?;

    results.sort_by_key(|r| std::cmp::Reverse(r.0));
    results.truncate(5);

    let mut output = format!("# Search Results for: {}\n\n", search_term);
    if results.is_empty() {
        output.push_str("No matches found.\n");
    } else {
        output.push_str("| Path | Score |\n");
        output.push_str("|------|-------|\n");
        for (score, path) in results {
            output.push_str(&format!("| `{}` | {} |\n", path, score));
        }
    }

    Ok(output)
}

fn handle_get_graph_relationships(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("path".to_string())) as Box<dyn std::error::Error>)?;

    let directory = args.get("directory")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    let file_name = Path::new(path)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    let mut outlinks = Vec::new();
    for line in content.lines() {
        let mut temp = line.to_string();
        while let Some(start) = temp.find("[[") {
            if let Some(end) = temp[start..].find("]]") {
                let link = &temp[start + 2..start + end];
                if !link.is_empty() && !outlinks.contains(&link.to_string()) {
                    outlinks.push(link.to_string());
                }
                temp = temp[start + end + 2..].to_string();
            } else {
                break;
            }
        }
    }

    let mut backlinks = Vec::new();
    fn scan_backlinks(dir: &Path, target_name: &str, links: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let _ = scan_backlinks(&path, target_name, links);
            } else if let Ok(content) = std::fs::read_to_string(&path) {
                if content.contains(&format!("[[{}]]", target_name)) {
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        if !links.contains(&file_name.to_string()) {
                            links.push(file_name.to_string());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    scan_backlinks(Path::new(directory), file_name, &mut backlinks)?;

    let mut output = format!("# Graph Relationships: {}\n\n", path);

    output.push_str("## Outlinks (references in this file)\n\n");
    if outlinks.is_empty() {
        output.push_str("No outlinks found.\n\n");
    } else {
        for link in &outlinks {
            output.push_str(&format!("- [[{}]]\n", link));
        }
        output.push('\n');
    }

    output.push_str("## Backlinks (files that reference this file)\n\n");
    if backlinks.is_empty() {
        output.push_str("No backlinks found.\n");
    } else {
        for link in &backlinks {
            output.push_str(&format!("- {}\n", link));
        }
    }

    Ok(output)
}

// ============================================================================
// Phase 4: AI/RAG toolkit
// ============================================================================

fn output_is_json(args: &Value) -> bool {
    args.get("output_format")
        .and_then(|v| v.as_str())
        .map(|f| f.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

/// Build the JSON representation of chunks for a given source.
fn chunks_to_json(chunks: &[rag::Chunk], source: &str) -> Value {
    let arr: Vec<Value> = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "id": format!("{}#{}", source, i),
                "text": c.text,
                "metadata": {
                    "source": source,
                    "heading_path": c.heading_path,
                    "chunk_index": i,
                    "token_estimate": c.token_estimate,
                }
            })
        })
        .collect();
    json!(arr)
}

fn handle_chunk_markdown(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(512) as usize;
    let overlap = args.get("overlap").and_then(|v| v.as_u64()).unwrap_or(64) as usize;

    let chunks = rag::chunk_markdown(&content, max_tokens, overlap);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&chunks_to_json(&chunks, &source))?);
    }

    let mut out = format!("# Chunks for: {}\n\n**{} chunks** (max_tokens={}, overlap={})\n\n", source, chunks.len(), max_tokens, overlap);
    for (i, c) in chunks.iter().enumerate() {
        let path = if c.heading_path.is_empty() { "(root)".to_string() } else { c.heading_path.join(" > ") };
        out.push_str(&format!("## Chunk {} — {} (~{} tokens)\n\n{}\n\n---\n\n", i, path, c.token_estimate, c.text));
    }
    Ok(out)
}

fn handle_extract_chunks_for_rag(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    // Same as chunk_markdown but defaults to JSON output for ingestion pipelines.
    let mut args = args.clone();
    if args.get("output_format").is_none() {
        args["output_format"] = json!("json");
    }
    handle_chunk_markdown(&args)
}

fn outline_to_json(nodes: &[rag::OutlineNode]) -> Value {
    let arr: Vec<Value> = nodes
        .iter()
        .map(|n| {
            json!({
                "level": n.level,
                "title": n.title,
                "anchor": n.anchor,
                "children": outline_to_json(&n.children),
            })
        })
        .collect();
    json!(arr)
}

fn outline_to_markdown(nodes: &[rag::OutlineNode], out: &mut String) {
    for n in nodes {
        out.push_str(&format!("{}- [{}](#{})\n", "  ".repeat(n.level.saturating_sub(1)), n.title, n.anchor));
        outline_to_markdown(&n.children, out);
    }
}

fn handle_get_document_outline(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let outline = rag::build_outline(&content);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&outline_to_json(&outline))?);
    }
    let mut out = format!("# Outline for: {}\n\n", source);
    if outline.is_empty() {
        out.push_str("(No headings found)\n");
    } else {
        outline_to_markdown(&outline, &mut out);
    }
    Ok(out)
}

fn handle_get_text_statistics(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let top_n = args.get("top_n").and_then(|v| v.as_u64()).unwrap_or(25) as usize;
    let min_length = args.get("min_length").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let exclude_stopwords = args.get("stopwords").and_then(|v| v.as_bool()).unwrap_or(false);

    let stats = rag::text_statistics(&content, exclude_stopwords, min_length);
    let richness = if stats.total_words > 0 {
        stats.distinct_words as f64 / stats.total_words as f64
    } else {
        0.0
    };
    let top: Vec<_> = stats.frequencies.iter().take(top_n).collect();

    if output_is_json(args) {
        let words: Vec<Value> = top
            .iter()
            .map(|(w, c)| json!({"word": w, "count": c, "pct": (*c as f64 / stats.total_words.max(1) as f64) * 100.0}))
            .collect();
        return Ok(serde_json::to_string_pretty(&json!({
            "source": source,
            "totals": {
                "total_words": stats.total_words,
                "distinct_words": stats.distinct_words,
                "vocabulary_richness": richness,
                "char_count": stats.char_count,
                "sentence_count": stats.sentence_count,
                "paragraph_count": stats.paragraph_count,
                "avg_words_per_sentence": stats.total_words as f64 / stats.sentence_count as f64,
            },
            "words": words,
        }))?);
    }

    let mut out = format!("# Text Statistics: {}\n\n", source);
    out.push_str(&format!("- **Total words:** {}\n", stats.total_words));
    out.push_str(&format!("- **Distinct words:** {}\n", stats.distinct_words));
    out.push_str(&format!("- **Vocabulary richness:** {:.3}\n", richness));
    out.push_str(&format!("- **Characters:** {}\n", stats.char_count));
    out.push_str(&format!("- **Sentences:** {}\n", stats.sentence_count));
    out.push_str(&format!("- **Paragraphs:** {}\n", stats.paragraph_count));
    out.push_str(&format!("- **Avg words/sentence:** {:.1}\n\n", stats.total_words as f64 / stats.sentence_count as f64));
    out.push_str("## Word Frequencies\n\n| Word | Count | % |\n| --- | --- | --- |\n");
    for (w, c) in top {
        out.push_str(&format!("| {} | {} | {:.2}% |\n", w, c, (*c as f64 / stats.total_words.max(1) as f64) * 100.0));
    }
    Ok(out)
}

fn handle_get_corpus_statistics(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory").and_then(|v| v.as_str()).unwrap_or(".");
    let exclude_stopwords = args.get("stopwords").and_then(|v| v.as_bool()).unwrap_or(true);
    let top_n = args.get("top_n").and_then(|v| v.as_u64()).unwrap_or(25) as usize;

    let files = collect_text_files(Path::new(directory));
    let mut global_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut per_doc: Vec<(String, usize, usize)> = Vec::new(); // (path, total, distinct)
    let mut corpus_total = 0usize;

    for file in &files {
        let content = match convert_any_to_markdown(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let stats = rag::text_statistics(&content, exclude_stopwords, 1);
        corpus_total += stats.total_words;
        for (w, c) in &stats.frequencies {
            *global_counts.entry(w.clone()).or_insert(0) += c;
        }
        per_doc.push((file.display().to_string(), stats.total_words, stats.distinct_words));
    }

    let global_distinct = global_counts.len();
    let mut top: Vec<(String, usize)> = global_counts.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top.truncate(top_n);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!({
            "directory": directory,
            "corpus": {
                "documents": per_doc.len(),
                "total_words": corpus_total,
                "global_distinct_words": global_distinct,
            },
            "documents": per_doc.iter().map(|(p, t, d)| json!({"path": p, "total_words": t, "distinct_words": d})).collect::<Vec<_>>(),
            "top_words": top.iter().map(|(w, c)| json!({"word": w, "count": c})).collect::<Vec<_>>(),
        }))?);
    }

    let mut out = format!("# Corpus Statistics: {}\n\n", directory);
    out.push_str(&format!("- **Documents:** {}\n- **Total words:** {}\n- **Global distinct words:** {}\n\n", per_doc.len(), corpus_total, global_distinct));
    out.push_str("## Per-document\n\n| File | Words | Distinct |\n| --- | --- | --- |\n");
    for (p, t, d) in &per_doc {
        out.push_str(&format!("| {} | {} | {} |\n", p, t, d));
    }
    out.push_str("\n## Top words (corpus-wide)\n\n| Word | Count |\n| --- | --- |\n");
    for (w, c) in &top {
        out.push_str(&format!("| {} | {} |\n", w, c));
    }
    Ok(out)
}

fn handle_search_content(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory").and_then(|v| v.as_str()).unwrap_or(".");
    let query = args.get("query").and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("query".to_string())) as Box<dyn std::error::Error>)?;
    let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let query_terms = rag::tokenize_words(query);
    let files = collect_text_files(Path::new(directory));

    let mut hits: Vec<(String, usize, String)> = Vec::new(); // (path, score, snippet)
    for file in &files {
        let content = match convert_any_to_markdown(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lower = content.to_lowercase();
        let score: usize = query_terms.iter().map(|t| lower.matches(t.as_str()).count()).sum();
        if score == 0 {
            continue;
        }
        let snippet = first_match_snippet(&content, &query_terms, 200);
        hits.push((file.display().to_string(), score, snippet));
    }
    hits.sort_by_key(|h| std::cmp::Reverse(h.1));
    hits.truncate(max_results);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!(hits.iter()
            .map(|(p, s, snip)| json!({"source": p, "score": s, "snippet": snip}))
            .collect::<Vec<_>>()))?);
    }

    let mut out = format!("# Content Search: `{}`\n\n**{} matching documents** in `{}`\n\n", query, hits.len(), directory);
    for (p, s, snip) in &hits {
        out.push_str(&format!("## {} (score {})\n\n{}\n\n", p, s, snip));
    }
    if hits.is_empty() {
        out.push_str("No matches found.\n");
    }
    Ok(out)
}

/// Find the first occurrence of any query term and return surrounding context.
fn first_match_snippet(content: &str, terms: &[String], window: usize) -> String {
    let lower = content.to_lowercase();
    let mut best: Option<usize> = None;
    for t in terms {
        if let Some(pos) = lower.find(t.as_str()) {
            best = Some(best.map_or(pos, |b| b.min(pos)));
        }
    }
    let Some(pos) = best else {
        return content.chars().take(window).collect();
    };
    let start = pos.saturating_sub(window / 2);
    let start = (0..=start).rev().find(|&i| content.is_char_boundary(i)).unwrap_or(0);
    let end = (pos + window / 2).min(content.len());
    let end = (end..=content.len()).find(|&i| content.is_char_boundary(i)).unwrap_or(content.len());
    let prefix = if start > 0 { "…" } else { "" };
    let suffix = if end < content.len() { "…" } else { "" };
    format!("{}{}{}", prefix, content[start..end].replace('\n', " ").trim(), suffix)
}

/// Recursively collect readable text/document files under a directory.
fn collect_text_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_text_files_inner(dir, &mut files, 0);
    files
}

fn collect_text_files_inner(dir: &Path, files: &mut Vec<std::path::PathBuf>, depth: usize) {
    if depth > 20 || files.len() > 10_000 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            collect_text_files_inner(&path, files, depth + 1);
        } else {
            files.push(path);
        }
    }
}

// ============================================================================
// Phase 5: "Second brain" knowledge tools
// ============================================================================

/// Collect only Markdown-like note files under a directory (for vault scans).
fn collect_note_files(dir: &Path) -> Vec<std::path::PathBuf> {
    collect_text_files(dir)
        .into_iter()
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(),
                Some("md" | "markdown" | "mdown" | "txt")
            )
        })
        .collect()
}

fn handle_extract_tags(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    // Directory scan (vault-wide tag index) or single file/content.
    if let Some(directory) = args.get("directory").and_then(|v| v.as_str()) {
        let files = collect_note_files(Path::new(directory));
        let mut tag_files: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for f in &files {
            if let Ok(content) = std::fs::read_to_string(f) {
                for tag in knowledge::extract_tags(&content) {
                    tag_files.entry(tag).or_default().push(f.display().to_string());
                }
            }
        }
        let mut index: Vec<(String, Vec<String>)> = tag_files.into_iter().collect();
        index.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

        if output_is_json(args) {
            return Ok(serde_json::to_string_pretty(&json!(index.iter().map(|(t, fs)| {
                let mut fs = fs.clone(); fs.sort(); fs.dedup();
                json!({"tag": t, "count": fs.len(), "files": fs})
            }).collect::<Vec<_>>()))?);
        }
        let mut out = format!("# Tag Index: {}\n\n| Tag | Count | Files |\n| --- | --- | --- |\n", directory);
        for (t, fs) in &index {
            let mut fs = fs.clone(); fs.sort(); fs.dedup();
            out.push_str(&format!("| #{} | {} | {} |\n", t, fs.len(), fs.join(", ")));
        }
        return Ok(out);
    }

    let (content, source) = resolve_content_arg(args)?;
    let ranked = knowledge::count_and_rank(knowledge::extract_tags(&content));
    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!(ranked.iter()
            .map(|(t, c)| json!({"tag": t, "count": c})).collect::<Vec<_>>()))?);
    }
    let mut out = format!("# Tags in: {}\n\n", source);
    if ranked.is_empty() { out.push_str("(No tags found)\n"); }
    for (t, c) in &ranked { out.push_str(&format!("- #{} ({})\n", t, c)); }
    Ok(out)
}

/// Build corpus document-frequency map over note files in a directory.
fn corpus_doc_freq(files: &[std::path::PathBuf]) -> (std::collections::HashMap<String, usize>, usize) {
    let mut df: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut n = 0;
    for f in files {
        if let Ok(content) = convert_any_to_markdown(f) {
            n += 1;
            for term in knowledge::term_frequencies(&content).keys() {
                *df.entry(term.clone()).or_insert(0) += 1;
            }
        }
    }
    (df, n)
}

fn handle_extract_keywords(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let top_n = args.get("top_n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let tf = knowledge::term_frequencies(&content);
    // Use the surrounding directory as the corpus for IDF when available.
    let (df, num_docs) = if let Some(directory) = args.get("directory").and_then(|v| v.as_str()) {
        corpus_doc_freq(&collect_note_files(Path::new(directory)))
    } else {
        (std::collections::HashMap::new(), 1)
    };
    let mut scores = knowledge::tf_idf(&tf, &df, num_docs);
    scores.truncate(top_n);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!(scores.iter()
            .map(|(t, s)| json!({"term": t, "score": s})).collect::<Vec<_>>()))?);
    }
    let mut out = format!("# Keywords in: {}\n\n", source);
    for (t, s) in &scores { out.push_str(&format!("- **{}** ({:.2})\n", t, s)); }
    Ok(out)
}

/// `embeddings: true` on the RAG tools switches from TF/SimHash heuristics to
/// vector similarity (fastembed model when compiled in, hashed vectors
/// otherwise).
fn embeddings_requested(args: &Value) -> bool {
    args.get("embeddings").and_then(Value::as_bool).unwrap_or(false)
}

fn boxed_err(e: anyhow::Error) -> Box<dyn std::error::Error> {
    Box::new(ConversionError::ConversionFailed(e.to_string()))
}

/// Build/refresh the vector index for `sources` and return it with the
/// embedder. The index persists under `<index_dir>/.tomarkdown/` when a
/// directory is given; single-file calls stay transient.
fn updated_vector_index(
    sources: &[std::path::PathBuf],
    index_dir: Option<&Path>,
) -> Result<(embeddings::VectorIndex, Box<dyn embeddings::Embedder>), Box<dyn std::error::Error>> {
    let mut embedder = embeddings::default_embedder();
    let mut index = match index_dir {
        Some(d) => embeddings::VectorIndex::load(d),
        None => embeddings::VectorIndex::default(),
    };
    index
        .update(sources, embedder.as_mut(), |p| convert_any_to_markdown(p).ok())
        .map_err(boxed_err)?;
    if let Some(d) = index_dir {
        if let Err(e) = index.save(d) {
            eprintln!("embeddings: could not persist index in {}: {}", d.display(), e);
        }
    }
    Ok((index, embedder))
}

/// Vector-similarity ranking for retrieve_context, mirroring rank_chunks.
fn embedding_scored_chunks(
    sources: &[std::path::PathBuf],
    index_dir: Option<&Path>,
    query: &str,
) -> Result<Vec<retrieval::ScoredChunk>, Box<dyn std::error::Error>> {
    let (index, mut embedder) = updated_vector_index(sources, index_dir)?;
    let qv = embedder.embed(&[query.to_string()]).map_err(boxed_err)?;
    Ok(index
        .rank(&qv[0])
        .into_iter()
        .filter(|(_, _, score)| *score > 0.0)
        .map(|(source, c, score)| retrieval::ScoredChunk {
            source,
            heading_path: c.heading_path.clone(),
            text: c.text.clone(),
            score: score as f64,
            token_estimate: c.token_estimate,
        })
        .collect())
}

fn handle_find_related_notes(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let file_path = args.get("file_path").and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("file_path".to_string())) as Box<dyn std::error::Error>)?;
    let directory = args.get("directory").and_then(|v| v.as_str()).unwrap_or(".");
    let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

    let target_content = convert_any_to_markdown(Path::new(file_path))?;
    let target_canon = std::fs::canonicalize(file_path).ok();

    if embeddings_requested(args) {
        // Vector path: rank notes by cosine over per-file mean chunk vectors.
        let sources = collect_note_files(Path::new(directory));
        let (index, mut embedder) = updated_vector_index(&sources, Some(Path::new(directory)))?;
        let tv = embedder.embed(std::slice::from_ref(&target_content)).map_err(boxed_err)?;
        let mut results: Vec<(String, f64)> = index
            .file_vectors()
            .into_iter()
            .filter(|(p, _)| std::fs::canonicalize(p).ok() != target_canon)
            .map(|(p, v)| (p, embeddings::cosine(&tv[0], &v) as f64))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(max_results);

        if output_is_json(args) {
            return Ok(serde_json::to_string_pretty(&json!(results.iter().map(|(p, s)| {
                json!({"path": p, "score": s, "method": "embeddings"})
            }).collect::<Vec<_>>()))?);
        }
        let mut out = format!("# Notes related to: {} (vector similarity)\n\n", file_path);
        if results.is_empty() { out.push_str("No related notes found.\n"); }
        for (p, s) in &results {
            out.push_str(&format!("- {} (similarity {:.3})\n", p, s));
        }
        return Ok(out);
    }

    let target_tf = knowledge::term_frequencies(&target_content);
    let target_tags: std::collections::HashSet<String> = knowledge::extract_tags(&target_content).into_iter().collect();

    let mut results: Vec<(String, f64, Vec<String>, Vec<String>)> = Vec::new();
    for f in collect_note_files(Path::new(directory)) {
        if std::fs::canonicalize(&f).ok() == target_canon {
            continue;
        }
        let content = match convert_any_to_markdown(&f) { Ok(c) => c, Err(_) => continue };
        let tf = knowledge::term_frequencies(&content);
        let mut score = knowledge::cosine_similarity(&target_tf, &tf);
        let tags: std::collections::HashSet<String> = knowledge::extract_tags(&content).into_iter().collect();
        let shared_tags: Vec<String> = target_tags.intersection(&tags).cloned().collect();
        score += 0.05 * shared_tags.len() as f64; // small boost for shared tags
        if score <= 0.0 { continue; }
        let shared = knowledge::shared_terms(&target_tf, &tf);
        results.push((f.display().to_string(), score, shared, shared_tags));
    }
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(max_results);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!(results.iter().map(|(p, s, terms, tags)| {
            json!({"path": p, "score": s, "shared_terms": terms.iter().take(10).collect::<Vec<_>>(), "shared_tags": tags})
        }).collect::<Vec<_>>()))?);
    }
    let mut out = format!("# Notes related to: {}\n\n", file_path);
    if results.is_empty() { out.push_str("No related notes found.\n"); }
    for (p, s, terms, _tags) in &results {
        out.push_str(&format!("## {} (similarity {:.3})\n\nShared terms: {}\n\n", p, s, terms.iter().take(8).cloned().collect::<Vec<_>>().join(", ")));
    }
    Ok(out)
}

fn handle_summarize_document(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let n = args.get("sentences").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let summary = knowledge::summarize(&content, n);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!({"source": source, "summary": summary}))?);
    }
    let mut out = format!("# Summary: {}\n\n", source);
    for s in &summary { out.push_str(&format!("- {}\n", s)); }
    Ok(out)
}

fn handle_extract_qa_pairs(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let pairs = knowledge::extract_qa_pairs(&content);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!(pairs.iter()
            .map(|(q, a)| json!({"question": q, "answer": a, "source": source}))
            .collect::<Vec<_>>()))?);
    }
    let mut out = format!("# Q&A Pairs: {}\n\n", source);
    if pairs.is_empty() { out.push_str("(No Q&A pairs found)\n"); }
    for (q, a) in &pairs {
        out.push_str(&format!("**Q:** {}\n\n**A:** {}\n\n---\n\n", q, if a.is_empty() { "_(no answer found)_" } else { a }));
    }
    Ok(out)
}

fn handle_extract_entities(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let entities = knowledge::extract_entities(&content);

    // Aggregate by (value, kind) with counts.
    let mut counts: std::collections::HashMap<(String, &'static str), usize> = std::collections::HashMap::new();
    for e in &entities { *counts.entry((e.value.clone(), e.kind)).or_insert(0) += 1; }
    let mut ranked: Vec<((String, &'static str), usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.0.cmp(&b.0.0)));

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!(ranked.iter()
            .map(|((v, k), c)| json!({"entity": v, "type": k, "count": c}))
            .collect::<Vec<_>>()))?);
    }
    let mut out = format!("# Entities: {}\n\n| Entity | Type | Count |\n| --- | --- | --- |\n", source);
    for ((v, k), c) in &ranked { out.push_str(&format!("| {} | {} | {} |\n", v, k, c)); }
    Ok(out)
}

fn handle_build_knowledge_index(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;

    // Compose the sub-analyses into one JSON knowledge artifact.
    let outline = rag::build_outline(&content);
    let chunks = rag::chunk_markdown(&content, 512, 64);
    let stats = rag::text_statistics(&content, true, 3);
    let tf = knowledge::term_frequencies(&content);
    let keywords = knowledge::tf_idf(&tf, &std::collections::HashMap::new(), 1);
    let tags = knowledge::count_and_rank(knowledge::extract_tags(&content));
    let summary = knowledge::summarize(&content, 3);

    let index = json!({
        "source": source,
        "summary": summary,
        "outline": outline_to_json(&outline),
        "tags": tags.iter().map(|(t, c)| json!({"tag": t, "count": c})).collect::<Vec<_>>(),
        "keywords": keywords.iter().take(15).map(|(t, s)| json!({"term": t, "score": s})).collect::<Vec<_>>(),
        "stats": {
            "total_words": stats.total_words,
            "distinct_words": stats.distinct_words,
        },
        "chunks": chunks_to_json(&chunks, &source),
    });

    // This tool is machine-oriented: always JSON.
    Ok(serde_json::to_string_pretty(&index)?)
}

// ============================================================================
// AI functions — Phase A: local RAG retrieval + token budgeting
// ============================================================================

fn handle_retrieve_context(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let query = args.get("query").and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("query".to_string())) as Box<dyn std::error::Error>)?;
    let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;
    let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(8) as usize;

    // Gather chunks from a single file or a whole directory.
    let (sources, index_dir): (Vec<std::path::PathBuf>, Option<&str>) =
        if let Some(fp) = args.get("file_path").and_then(|v| v.as_str()) {
            (vec![std::path::PathBuf::from(fp)], None)
        } else {
            let dir = args.get("directory").and_then(|v| v.as_str()).unwrap_or(".");
            (collect_text_files(Path::new(dir)), Some(dir))
        };

    let ranked = if embeddings_requested(args) {
        embedding_scored_chunks(&sources, index_dir.map(Path::new), query)?
    } else {
        let mut all_chunks: Vec<(String, rag::Chunk)> = Vec::new();
        for path in &sources {
            let content = match convert_any_to_markdown(path) { Ok(c) => c, Err(_) => continue };
            for c in rag::chunk_markdown(&content, 512, 64) {
                all_chunks.push((path.display().to_string(), c));
            }
        }
        retrieval::rank_chunks(query, all_chunks)
    };
    let selected = retrieval::select_within_budget(ranked, max_tokens, top_k);
    let context = retrieval::assemble_context(&selected);
    let used_tokens: usize = selected.iter().map(|c| c.token_estimate).sum();

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!({
            "query": query,
            "used_tokens": used_tokens,
            "context": context,
            "chunks": selected.iter().enumerate().map(|(i, c)| json!({
                "rank": i + 1,
                "source": c.source,
                "heading_path": c.heading_path,
                "score": c.score,
                "token_estimate": c.token_estimate,
                "text": c.text,
            })).collect::<Vec<_>>(),
        }))?);
    }

    let mut out = format!("# Retrieved context for: `{}`\n\n", query);
    out.push_str(&format!("**{} chunks, ~{} tokens** (budget {})\n\n", selected.len(), used_tokens, max_tokens));
    if selected.is_empty() {
        out.push_str("No relevant content found.\n");
        return Ok(out);
    }
    out.push_str("## Context\n\n");
    out.push_str(&context);
    out.push_str("\n\n## Citations\n\n");
    for (i, c) in selected.iter().enumerate() {
        let loc = if c.heading_path.is_empty() { c.source.clone() } else { format!("{} › {}", c.source, c.heading_path.join(" › ")) };
        out.push_str(&format!("{}. {} (score {:.2})\n", i + 1, loc, c.score));
    }
    Ok(out)
}

fn handle_count_tokens(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let tokens = rag::estimate_tokens(&content);
    let words = content.split_whitespace().count();
    let chars = content.chars().count();

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!({
            "source": source,
            "estimated_tokens": tokens,
            "words": words,
            "chars": chars,
            "models": retrieval::model_context_windows().iter().map(|(m, w)| json!({
                "model": m,
                "context_window": w,
                "fits": tokens <= *w,
                "pct_of_window": (tokens as f64 / *w as f64) * 100.0,
            })).collect::<Vec<_>>(),
        }))?);
    }

    let mut out = format!("# Token Estimate: {}\n\n", source);
    out.push_str(&format!("- **Estimated tokens:** ~{}\n- **Words:** {}\n- **Characters:** {}\n\n", tokens, words, chars));
    out.push_str("## Context-window fit\n\n| Model | Window | Fits | % of window |\n| --- | --- | --- | --- |\n");
    for (m, w) in retrieval::model_context_windows() {
        out.push_str(&format!("| {} | {} | {} | {:.2}% |\n", m, w, if tokens <= w { "✅" } else { "❌" }, (tokens as f64 / w as f64) * 100.0));
    }
    out.push_str("\n_Token counts are heuristic estimates (~words × 1.3), not exact tokenizer counts._\n");
    Ok(out)
}

// ============================================================================
// AI functions — Phase B: dedup & clustering
// ============================================================================

fn handle_find_duplicates(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory").and_then(|v| v.as_str()).unwrap_or(".");
    // Bits (out of 64) allowed to differ; lower = stricter. Default ~5% of bits.
    let threshold = args.get("threshold").and_then(|v| v.as_u64()).unwrap_or(3) as u32;

    let files = collect_text_files(Path::new(directory));

    if embeddings_requested(args) {
        // Vector path: group files whose mean chunk vectors exceed
        // min_similarity (cosine, default 0.9) with a greedy pass.
        let min_similarity = args.get("min_similarity").and_then(|v| v.as_f64()).unwrap_or(0.9);
        let (index, _) = updated_vector_index(&files, Some(Path::new(directory)))?;
        let vectors = index.file_vectors();
        let mut grouped = vec![false; vectors.len()];
        let mut groups: Vec<Vec<(String, f64)>> = Vec::new();
        for i in 0..vectors.len() {
            if grouped[i] { continue; }
            let mut group = vec![(vectors[i].0.clone(), 1.0f64)];
            for j in (i + 1)..vectors.len() {
                if grouped[j] { continue; }
                let sim = embeddings::cosine(&vectors[i].1, &vectors[j].1) as f64;
                if sim >= min_similarity {
                    grouped[j] = true;
                    group.push((vectors[j].0.clone(), sim));
                }
            }
            if group.len() > 1 {
                grouped[i] = true;
                groups.push(group);
            }
        }

        if output_is_json(args) {
            return Ok(serde_json::to_string_pretty(&json!(groups.iter().map(|g| {
                json!(g.iter().map(|(p, s)| json!({"path": p, "similarity": s})).collect::<Vec<_>>())
            }).collect::<Vec<_>>()))?);
        }
        let mut out = format!("# Near-duplicate groups in: {} (vector similarity)\n\n**{} group(s)** (min similarity {:.2}, {} files scanned)\n\n", directory, groups.len(), min_similarity, vectors.len());
        if groups.is_empty() { out.push_str("No near-duplicates found.\n"); }
        for (i, g) in groups.iter().enumerate() {
            out.push_str(&format!("## Group {}\n\n", i + 1));
            for (p, s) in g {
                out.push_str(&format!("- {} ({:.1}% similar)\n", p, s * 100.0));
            }
            out.push('\n');
        }
        return Ok(out);
    }

    let mut prints: Vec<(String, u64)> = Vec::new();
    for f in &files {
        if let Ok(content) = convert_any_to_markdown(f) {
            prints.push((f.display().to_string(), similarity::simhash(&content)));
        }
    }
    let groups = similarity::group_near_duplicates(&prints, threshold);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!(groups.iter().map(|g| {
            json!(g.iter().map(|(p, s)| json!({"path": p, "similarity": s})).collect::<Vec<_>>())
        }).collect::<Vec<_>>()))?);
    }
    let mut out = format!("# Near-duplicate groups in: {}\n\n**{} group(s)** (threshold {} bits, {} files scanned)\n\n", directory, groups.len(), threshold, prints.len());
    if groups.is_empty() {
        out.push_str("No near-duplicates found.\n");
    }
    for (i, g) in groups.iter().enumerate() {
        out.push_str(&format!("## Group {}\n\n", i + 1));
        for (p, s) in g {
            out.push_str(&format!("- {} ({:.1}% similar)\n", p, s * 100.0));
        }
        out.push('\n');
    }
    Ok(out)
}

fn handle_cluster_documents(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let directory = args.get("directory").and_then(|v| v.as_str()).unwrap_or(".");
    let min_similarity = args.get("min_similarity").and_then(|v| v.as_f64()).unwrap_or(0.2);

    let files = collect_text_files(Path::new(directory));

    if embeddings_requested(args) {
        // Vector path: greedy clustering by cosine over per-file vectors;
        // labels come from the seed file's stem (no term vectors here).
        let (index, _) = updated_vector_index(&files, Some(Path::new(directory)))?;
        let vectors = index.file_vectors();
        let mut grouped = vec![false; vectors.len()];
        let mut clusters: Vec<(String, Vec<String>)> = Vec::new();
        for i in 0..vectors.len() {
            if grouped[i] { continue; }
            grouped[i] = true;
            let mut members = vec![vectors[i].0.clone()];
            for j in (i + 1)..vectors.len() {
                if grouped[j] { continue; }
                if (embeddings::cosine(&vectors[i].1, &vectors[j].1) as f64) >= min_similarity {
                    grouped[j] = true;
                    members.push(vectors[j].0.clone());
                }
            }
            let label = Path::new(&vectors[i].0)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("cluster {}", clusters.len() + 1));
            clusters.push((label, members));
        }

        if output_is_json(args) {
            return Ok(serde_json::to_string_pretty(&json!(clusters.iter().map(|(label, members)| json!({
                "label": label,
                "members": members,
                "method": "embeddings",
            })).collect::<Vec<_>>()))?);
        }
        let mut out = format!("# Document clusters in: {} (vector similarity)\n\n**{} cluster(s)** (min similarity {:.2})\n\n", directory, clusters.len(), min_similarity);
        for (label, members) in &clusters {
            out.push_str(&format!("## {} ({} docs)\n\n", label, members.len()));
            for m in members {
                out.push_str(&format!("- {}\n", m));
            }
            out.push('\n');
        }
        return Ok(out);
    }

    let mut docs: Vec<similarity::Document> = Vec::new();
    for f in &files {
        if let Ok(content) = convert_any_to_markdown(f) {
            docs.push(similarity::Document::new(&f.display().to_string(), &content));
        }
    }
    let clusters = similarity::cluster_documents(&docs, min_similarity);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!(clusters.iter().map(|c| json!({
            "label": c.label,
            "members": c.members,
            "top_terms": c.top_terms,
        })).collect::<Vec<_>>()))?);
    }
    let mut out = format!("# Document clusters in: {}\n\n**{} cluster(s)** (min similarity {:.2})\n\n", directory, clusters.len(), min_similarity);
    for c in &clusters {
        out.push_str(&format!("## {} ({} docs)\n\nTop terms: {}\n\n", c.label, c.members.len(), c.top_terms.join(", ")));
        for m in &c.members {
            out.push_str(&format!("- {}\n", m));
        }
        out.push('\n');
    }
    Ok(out)
}

// ============================================================================
// AI functions — Phase C: document intelligence
// ============================================================================

fn handle_analyze_readability(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let r = doc_intel::readability(&content);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!({
            "source": source,
            "words": r.words,
            "sentences": r.sentences,
            "syllables": r.syllables,
            "avg_sentence_length": r.avg_sentence_length,
            "flesch_reading_ease": r.flesch_reading_ease,
            "flesch_kincaid_grade": r.flesch_kincaid_grade,
            "interpretation": doc_intel::flesch_interpretation(r.flesch_reading_ease),
        }))?);
    }
    let mut out = format!("# Readability: {}\n\n", source);
    out.push_str(&format!("- **Flesch Reading Ease:** {:.1} — {}\n", r.flesch_reading_ease, doc_intel::flesch_interpretation(r.flesch_reading_ease)));
    out.push_str(&format!("- **Flesch-Kincaid Grade:** {:.1}\n", r.flesch_kincaid_grade));
    out.push_str(&format!("- **Words:** {}\n- **Sentences:** {}\n- **Syllables:** {}\n- **Avg sentence length:** {:.1} words\n", r.words, r.sentences, r.syllables, r.avg_sentence_length));
    Ok(out)
}

fn handle_detect_natural_language(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let guesses = doc_intel::detect_language(&content);
    let top_n = args.get("top_n").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let top: Vec<_> = guesses.iter().take(top_n).collect();

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!({
            "source": source,
            "detected": top.first().map(|g| g.language),
            "candidates": top.iter().map(|g| json!({"language": g.language, "confidence": g.confidence})).collect::<Vec<_>>(),
        }))?);
    }
    let mut out = format!("# Language detection: {}\n\n", source);
    if let Some(best) = top.first() {
        out.push_str(&format!("**Detected:** {} (confidence {:.2})\n\n", best.language, best.confidence));
    }
    out.push_str("| Language | Confidence |\n| --- | --- |\n");
    for g in &top {
        out.push_str(&format!("| {} | {:.3} |\n", g.language, g.confidence));
    }
    Ok(out)
}

fn handle_classify_document(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let (content, source) = resolve_content_arg(args)?;
    let scores = doc_intel::classify(&content);

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!({
            "source": source,
            "top_category": scores.first().map(|c| c.category),
            "categories": scores.iter().map(|c| json!({"category": c.category, "score": c.score})).collect::<Vec<_>>(),
        }))?);
    }
    let mut out = format!("# Classification: {}\n\n", source);
    if let Some(top) = scores.first() {
        out.push_str(&format!("**Top category:** {} ({:.2})\n\n", top.category, top.score));
    } else {
        out.push_str("No strong category signals found.\n\n");
    }
    out.push_str("| Category | Score |\n| --- | --- |\n");
    for c in &scores {
        out.push_str(&format!("| {} | {:.2} |\n", c.category, c.score));
    }
    Ok(out)
}

// ============================================================================
// AI functions — Phase D: optional Claude-backed generative tools
// ============================================================================

/// Cap content length fed to the model to keep requests bounded.
fn truncate_for_prompt(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }
    let end = (0..=max_chars).rev().find(|&i| content.is_char_boundary(i)).unwrap_or(0);
    format!("{}\n\n[...truncated...]", &content[..end])
}

async fn handle_ai_summarize(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    if llm::api_key().is_none() {
        return Ok(llm::no_key_note("ai_summarize"));
    }
    let (content, _source) = resolve_content_arg(args)?;
    let style = args.get("style").and_then(|v| v.as_str()).unwrap_or("concise");
    let model = llm::resolve_model(args.get("model").and_then(|v| v.as_str()));
    let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(600) as u32;

    let prompt = format!(
        "Summarize the following document in a {} style. Use Markdown.\n\n---\n{}",
        style,
        truncate_for_prompt(&content, 40_000)
    );
    llm::complete(&prompt, Some("You are a precise summarization assistant."), &model, max_tokens)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)
}

async fn handle_ai_ask(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    if llm::api_key().is_none() {
        return Ok(llm::no_key_note("ai_ask"));
    }
    let question = args.get("question").and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("question".to_string())) as Box<dyn std::error::Error>)?;
    let model = llm::resolve_model(args.get("model").and_then(|v| v.as_str()));
    let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(800) as u32;

    // Build retrieval context (reuse the same gathering as retrieve_context).
    let mut retrieval_args = args.clone();
    retrieval_args["query"] = json!(question);
    if retrieval_args.get("max_tokens").is_none() {
        retrieval_args["max_tokens"] = json!(3000);
    }
    retrieval_args["output_format"] = json!("json");
    let ctx_json = handle_retrieve_context(&retrieval_args)?;
    let parsed: Value = serde_json::from_str(&ctx_json)?;
    let context = parsed.get("context").and_then(|v| v.as_str()).unwrap_or("");
    let citations = parsed.get("chunks").cloned().unwrap_or(json!([]));

    let prompt = format!(
        "Answer the question using ONLY the context below. Cite sources by their [n] markers. \
         If the context does not contain the answer, say so.\n\n## Context\n{}\n\n## Question\n{}",
        context, question
    );
    let answer = llm::complete(&prompt, Some("You are a helpful research assistant that grounds answers in provided context."), &model, max_tokens)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    if output_is_json(args) {
        return Ok(serde_json::to_string_pretty(&json!({
            "question": question,
            "answer": answer,
            "citations": citations,
        }))?);
    }
    let mut out = format!("# {}\n\n{}\n\n## Sources\n\n", question, answer);
    if let Some(chunks) = citations.as_array() {
        for c in chunks {
            let src = c.get("source").and_then(|v| v.as_str()).unwrap_or("?");
            let rank = c.get("rank").and_then(|v| v.as_u64()).unwrap_or(0);
            out.push_str(&format!("{}. {}\n", rank, src));
        }
    }
    Ok(out)
}

async fn handle_ai_tag(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    if llm::api_key().is_none() {
        return Ok(llm::no_key_note("ai_tag"));
    }
    let (content, _source) = resolve_content_arg(args)?;
    let max_tags = args.get("max_tags").and_then(|v| v.as_u64()).unwrap_or(8);
    let model = llm::resolve_model(args.get("model").and_then(|v| v.as_str()));

    let prompt = format!(
        "Suggest up to {} concise topical tags (lowercase, hyphenated, no '#') for this document. \
         Return ONLY a comma-separated list.\n\n---\n{}",
        max_tags,
        truncate_for_prompt(&content, 20_000)
    );
    llm::complete(&prompt, Some("You are a librarian that assigns consistent topical tags."), &model, 200)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)
}

async fn handle_ai_translate(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    if llm::api_key().is_none() {
        return Ok(llm::no_key_note("ai_translate"));
    }
    let (content, _source) = resolve_content_arg(args)?;
    let target = args.get("target_language").and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("target_language".to_string())) as Box<dyn std::error::Error>)?;
    let model = llm::resolve_model(args.get("model").and_then(|v| v.as_str()));
    let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(2000) as u32;

    let prompt = format!(
        "Translate the following document into {}. Preserve Markdown formatting. Output only the \
         translation.\n\n---\n{}",
        target,
        truncate_for_prompt(&content, 30_000)
    );
    llm::complete(&prompt, Some("You are a professional translator."), &model, max_tokens)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)
}

async fn handle_ai_classify(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    if llm::api_key().is_none() {
        return Ok(llm::no_key_note("ai_classify"));
    }
    let (content, _source) = resolve_content_arg(args)?;
    let labels: Vec<String> = args.get("labels")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    if labels.is_empty() {
        return Err(Box::new(ConversionError::MissingParameter("labels (array)".to_string())) as Box<dyn std::error::Error>);
    }
    let model = llm::resolve_model(args.get("model").and_then(|v| v.as_str()));

    let prompt = format!(
        "Classify the following document into exactly one of these labels: {}. Respond with the \
         label and a one-sentence justification.\n\n---\n{}",
        labels.join(", "),
        truncate_for_prompt(&content, 20_000)
    );
    llm::complete(&prompt, Some("You are a precise document classifier."), &model, 200)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)
}

#[cfg(test)]
mod base_dir_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_vault() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_vault")
    }

    #[test]
    fn absolute_paths_untouched() {
        let dirs = vec![fixture_vault()];
        assert_eq!(resolve_with(&dirs, "/etc/hosts"), "/etc/hosts");
    }

    #[test]
    fn relative_path_resolves_to_existing_dir() {
        let dirs = vec![PathBuf::from("/nonexistent-vault"), fixture_vault()];
        let resolved = resolve_with(&dirs, "Note A.md");
        assert!(resolved.ends_with("mini_vault/Note A.md"), "{}", resolved);
        assert!(std::path::Path::new(&resolved).exists());
    }

    #[test]
    fn missing_relative_falls_back_to_primary() {
        let dirs = vec![fixture_vault(), PathBuf::from("/other")];
        let resolved = resolve_with(&dirs, "brand-new-note.md");
        assert!(resolved.starts_with(fixture_vault().to_str().unwrap()), "{}", resolved);
    }

    #[test]
    fn urls_and_stdin_untouched() {
        let dirs = vec![fixture_vault()];
        assert_eq!(resolve_with(&dirs, "https://example.com/a.md"), "https://example.com/a.md");
        assert_eq!(resolve_with(&dirs, "-"), "-");
    }

    #[test]
    fn no_dirs_is_noop() {
        assert_eq!(resolve_with(&[], "notes/a.md"), "notes/a.md");
        let mut args = serde_json::json!({"file_path": "notes/a.md"});
        apply_base_dirs_with(&[], &mut args);
        assert_eq!(args["file_path"], "notes/a.md");
        assert!(args.get("vault_path").is_none());
    }

    #[test]
    fn injects_missing_vault_path_and_directory() {
        let dirs = vec![fixture_vault()];
        let mut args = serde_json::json!({});
        apply_base_dirs_with(&dirs, &mut args);
        assert_eq!(args["vault_path"].as_str().unwrap(), fixture_vault().to_str().unwrap());
        assert_eq!(args["directory"].as_str().unwrap(), fixture_vault().to_str().unwrap());
    }

    #[test]
    fn explicit_vault_path_not_overwritten() {
        let dirs = vec![fixture_vault()];
        let mut args = serde_json::json!({"vault_path": "/explicit/vault"});
        apply_base_dirs_with(&dirs, &mut args);
        assert_eq!(args["vault_path"], "/explicit/vault");
    }

    #[test]
    fn file_paths_array_resolved() {
        let dirs = vec![fixture_vault()];
        let mut args = serde_json::json!({"file_paths": ["Note A.md", "/abs/x.md"]});
        apply_base_dirs_with(&dirs, &mut args);
        let arr = args["file_paths"].as_array().unwrap();
        assert!(arr[0].as_str().unwrap().ends_with("mini_vault/Note A.md"));
        assert_eq!(arr[1], "/abs/x.md");
    }

    #[test]
    fn source_url_kept_source_path_resolved() {
        let dirs = vec![fixture_vault()];
        let mut args = serde_json::json!({"source": "https://example.com"});
        apply_base_dirs_with(&dirs, &mut args);
        assert_eq!(args["source"], "https://example.com");
        let mut args = serde_json::json!({"source": "Note A.md"});
        apply_base_dirs_with(&dirs, &mut args);
        assert!(args["source"].as_str().unwrap().ends_with("mini_vault/Note A.md"));
    }

    #[test]
    fn large_files_stream_or_are_refused() {
        let dir = std::env::temp_dir().join(format!("large_file_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 11 MB of plain text: must stream to a fenced block, not error.
        let txt = dir.join("big.log");
        std::fs::write(&txt, "line of log output\n".repeat(600_000)).unwrap();
        assert!(std::fs::metadata(&txt).unwrap().len() > LARGE_FILE_BYTES);
        let out = handle_convert_file(&json!({"file_path": txt.to_str().unwrap()})).unwrap();
        assert!(out.contains("```"));
        assert!(out.contains("line of log output"));
        // Same file with a lowered max_bytes and line numbers still streams.
        let out = handle_convert_file(&json!({
            "file_path": txt.to_str().unwrap(), "max_bytes": 1024, "add_line_numbers": true
        })).unwrap();
        assert!(out.contains("   1 | line of log output"));
        // An oversized structured file is refused with guidance.
        let html = dir.join("big.html");
        std::fs::write(&html, "<p>x</p>".repeat(2_000_000)).unwrap();
        let err = handle_convert_file(&json!({"file_path": html.to_str().unwrap()}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("max_bytes"), "unexpected error: {}", err);
        // convert_any_to_markdown applies the same gate for structured files.
        let err = convert_any_to_markdown(&html).unwrap_err().to_string();
        assert!(err.contains("limit"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn collect_resources_lists_vault_files() {
        let mut out = Vec::new();
        collect_resources(&fixture_vault(), &mut out);
        assert!(!out.is_empty(), "fixture vault should yield resources");
        let names: Vec<&str> = out.iter().filter_map(|r| r["name"].as_str()).collect();
        assert!(names.iter().any(|n| n.contains("Note A.md")), "names: {:?}", names);
        for r in &out {
            assert!(r["uri"].as_str().unwrap().starts_with("file://"));
            assert!(r["mimeType"].is_string());
        }
    }

    #[test]
    fn resource_path_rejects_escapes_and_bad_schemes() {
        let dirs = vec![fixture_vault().canonicalize().unwrap()];
        // Valid file inside the vault resolves.
        let uri = format!("file://{}", fixture_vault().join("Note A.md").display());
        assert!(resource_path_in(&dirs, &uri).is_ok());
        // Outside the vault is rejected even though the file exists.
        let outside = format!("file://{}", PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml").display());
        assert!(resource_path_in(&dirs, &outside).is_err());
        // Traversal out of the vault is rejected after canonicalization.
        let escape = format!("file://{}", fixture_vault().join("../../../Cargo.toml").display());
        assert!(resource_path_in(&dirs, &escape).is_err());
        // Non-file scheme and no-base-dir cases are rejected.
        assert!(resource_path_in(&dirs, "https://example.com/x.md").is_err());
        assert!(resource_path_in(&[], &uri).is_err());
    }

    #[test]
    fn prompts_render_and_validate() {
        // Every listed prompt renders with its required args.
        let (_, text) = render_prompt("summarize_note", &json!({"path": "Note A.md"})).unwrap();
        assert!(text.contains("Note A.md"));
        let (_, text) = render_prompt("ingest_url", &json!({"url": "https://example.com", "save_path": "web/page.md"})).unwrap();
        assert!(text.contains("https://example.com") && text.contains("web/page.md"));
        let (_, text) = render_prompt("vault_health", &json!({})).unwrap();
        assert!(text.contains("obsidian_vault_index"));
        // Missing required arg and unknown prompt fail.
        assert!(render_prompt("summarize_note", &json!({})).is_err());
        assert!(render_prompt("nope", &json!({})).is_err());
        // prompts/list definitions all render (with dummy required args).
        for p in prompt_definitions().as_array().unwrap() {
            let name = p["name"].as_str().unwrap();
            let mut args = serde_json::Map::new();
            for a in p["arguments"].as_array().unwrap() {
                if a["required"].as_bool().unwrap_or(false) {
                    args.insert(a["name"].as_str().unwrap().to_string(), json!("x"));
                }
            }
            assert!(render_prompt(name, &Value::Object(args)).is_ok(), "prompt {} failed", name);
        }
    }

    #[test]
    fn schema_help_covers_every_tool() {
        let defs = tool_definitions();
        for tool in defs.as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            let help = schema_help(name).unwrap_or_else(|| panic!("no schema help for {}", name));
            assert!(!help.trim().is_empty(), "empty schema help for {}", name);
        }
        assert!(schema_help("definitely_not_a_tool").is_none());
    }

    #[test]
    fn cli_structure_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_parses_base_dir_with_subcommand() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["to_markdown_mcp", "--base-dir", "/tmp", "tui", "some/path"]).unwrap();
        assert_eq!(cli.base_dir, vec!["/tmp".to_string()]);
        assert!(matches!(cli.command, Some(CliCommand::Tui { path: Some(ref p) }) if p == "some/path"));

        // Comma-separated values expand to multiple dirs; global flag also
        // parses after the subcommand.
        let cli = Cli::try_parse_from(["to_markdown_mcp", "tui", "--base-dir=/a,/b"]).unwrap();
        assert_eq!(cli.base_dir, vec!["/a".to_string(), "/b".to_string()]);
    }

    #[test]
    fn cli_no_args_means_server_mode() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["to_markdown_mcp"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn cli_parses_convert_flags() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "to_markdown_mcp", "convert", "script.py",
            "-o", "out.md", "--type", "python", "--line-numbers",
        ]).unwrap();
        match cli.command {
            Some(CliCommand::Convert { source, output, file_type, line_numbers, title }) => {
                assert_eq!(source, "script.py");
                assert_eq!(output.unwrap().to_str().unwrap(), "out.md");
                assert_eq!(file_type.as_deref(), Some("python"));
                assert!(line_numbers);
                assert!(title.is_none());
            }
            _ => panic!("expected Convert subcommand"),
        }
    }
}
