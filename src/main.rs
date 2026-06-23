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
