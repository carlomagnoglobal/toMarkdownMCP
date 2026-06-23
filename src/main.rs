use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

mod converter;
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
    id: String,
    method: String,
    params: Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: String,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
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
                let response = handle_request(&request).await;
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
            }
            Err(e) => {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": "unknown",
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

async fn handle_request(request: &JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "tools/list" => handle_list_tools(&request.id),
        "tools/call" => handle_call_tool(request).await,
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        },
    }
}

fn handle_list_tools(id: &str) -> JsonRpcResponse {
    let tools = json!([
        {
            "name": "convert_file",
            "description": "Convert a text, code, or HTML file to Markdown format",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to convert"
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
        }
    ]);

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: id.to_string(),
        result: Some(json!({
            "tools": tools
        })),
        error: None,
    }
}

async fn handle_call_tool(request: &JsonRpcRequest) -> JsonRpcResponse {
    let tool_name = request.params.get("name").and_then(|v| v.as_str());
    let arguments = request.params.get("arguments").cloned().unwrap_or(Value::Object(Default::default()));

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
        _ => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
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
            id: request.id.clone(),
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
        Err(e) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32603,
                message: "Internal error".to_string(),
                data: Some(json!(e.to_string())),
            }),
        },
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

    if is_html {
        // Handle HTML conversion
        return handle_html_file_conversion(file_path, filename, extract_metadata, preserve_css_hints, generate_toc_flag, toc_max_level, extract_images, image_format, convert_tables, extract_forms, preserve_comments, extract_links, analyze_headings, extract_definition_lists, extract_blockquotes);
    }

    // Read file
    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

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

    // Parse source
    let source = SourceType::from_string(source_str)
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

    // Fetch content
    let content = fetch_from_source(&source)
        .await
        .map_err(|e| Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>)?;

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
            let mut html_content = content.clone();
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

            let mut html_markdown = html_to_markdown_with_options(&html_content, extract_metadata, preserve_css_hints)
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
                prefixed
            } else {
                html_markdown
            }
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
    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    let extension = path.extension().and_then(|ext| ext.to_str());
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
        let mut cleaned = content.replace('<', " ").replace('>', " ");
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
                        let start = if pos > context_chars { pos - context_chars } else { 0 };
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
                                        if !tag.is_empty() && tag.chars().next().map_or(false, |c| c.is_alphabetic()) {
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
                            todos_by_file.entry(path_str).or_insert_with(Vec::new).push(line.trim().to_string());
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

    for idx in start_idx..lines.len() {
        if let Some(stripped) = lines[idx].strip_prefix('#') {
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
    table.push_str("|");
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
            let mut found_table = false;
            for idx in (h_idx + 1)..lines.len() {
                if lines[idx].starts_with('|') {
                    let table_start = idx;
                    let mut table_end = idx + 1;
                    while table_end < lines.len() && lines[table_end].starts_with('|') {
                        table_end += 1;
                    }
                    found_table = true;

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

            if !found_table {
                let mut new_lines = lines.clone();
                new_lines.insert(h_idx + 1, "");
                new_lines.insert(h_idx + 2, &table);
                std::fs::write(file_path, new_lines.join("\n"))
                    .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
                return Ok(success_msg);
            }
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
            _ => {
                help.push_str("Unknown tool. Use get_tool_help without tool_name to see all tools.\n");
            }
        }
    } else {
        // Summary of all tools
        help.push_str("# Available Tools\n\n");
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
        help.push_str("| get_graph_relationships | Map wiki-link connections |\n\n");
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
        if !recursive {
            if path_obj.read_dir()
                .map(|mut d| d.next().is_some())
                .unwrap_or(false)
            {
                return Err("Directory is not empty. Set recursive=true to delete non-empty directories".into());
            }
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
    let (yaml_start, yaml_end) = if lines.len() > 0 && lines[0] == "---" {
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
        for i in 1..end {
            yaml_lines.push(lines[i].to_string());
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
        for i in (end + 1)..lines.len() {
            new_content.push_str(lines[i]);
            new_content.push('\n');
        }
    } else if lines.len() > 0 {
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

    results.sort_by(|a, b| b.0.cmp(&a.0));
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
