use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

mod converter;
mod file_type;
mod error;

use converter::convert_to_markdown;
use file_type::detect_language;
use error::ConversionError;

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
            "description": "Convert a text or code file to Markdown format",
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
                    }
                },
                "required": ["content"]
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

    let path = Path::new(file_path);

    // Read file
    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;

    // Detect language from file extension
    let language = detect_language(path);

    // Get filename if needed
    let filename = if include_filename {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("File")
    } else {
        ""
    };

    Ok(convert_to_markdown(&content, Some(&language), Some(filename)))
}

fn handle_convert_text(args: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let content = args.get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Box::new(ConversionError::MissingParameter("content".to_string())) as Box<dyn std::error::Error>)?;

    let file_type = args.get("file_type")
        .and_then(|v| v.as_str());

    let title = args.get("title")
        .and_then(|v| v.as_str());

    Ok(convert_to_markdown(content, file_type, title))
}
