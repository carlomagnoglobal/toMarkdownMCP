//! End-to-end JSON-RPC tests: one tools/call per tool family, driven through
//! the real binary over stdio. Browser tools are excluded (need Chromium);
//! ai_* tools are exercised for their graceful no-API-key path.

use std::io::Write;
use std::process::{Command, Stdio};

fn vault() -> String {
    format!("{}/tests/fixtures/mini_vault", env!("CARGO_MANIFEST_DIR"))
}

/// Send JSON-RPC lines to the server and collect one parsed response per id.
fn call(requests: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_to_markdown_mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("server starts");
    let mut stdin = child.stdin.take().unwrap();
    for r in requests {
        writeln!(stdin, "{}", r).unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().expect("server exits");
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("valid JSON response"))
        .collect()
}

fn tool_call(id: u64, name: &str, args: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": name, "arguments": args}
    })
}

/// Assert the response with this id is a non-empty text result, and return it.
fn expect_text(responses: &[serde_json::Value], id: u64) -> String {
    let r = responses
        .iter()
        .find(|r| r["id"] == serde_json::json!(id))
        .unwrap_or_else(|| panic!("no response for id {}", id));
    assert!(
        r.get("error").is_none(),
        "id {} returned error: {}",
        id,
        r["error"]
    );
    let text = r["result"]["content"][0]["text"].as_str().unwrap_or_default();
    assert!(!text.trim().is_empty(), "id {} returned empty text", id);
    text.to_string()
}

#[test]
fn every_tool_family_answers_over_jsonrpc() {
    let vault = vault();
    let note = format!("{}/Note A.md", vault);
    let requests = vec![
        // conversion family
        tool_call(1, "convert_text", serde_json::json!({"content": "hello", "file_type": "python"})),
        tool_call(2, "convert_file", serde_json::json!({"file_path": note})),
        // file/vault ops family
        tool_call(3, "list_directory_files", serde_json::json!({"directory": vault})),
        tool_call(4, "get_file_summary", serde_json::json!({"file_path": note})),
        tool_call(5, "get_vault_statistics", serde_json::json!({"directory": vault})),
        // markdown/document editing family
        tool_call(6, "get_document_outline", serde_json::json!({"file_path": note})),
        tool_call(7, "extract_active_todos", serde_json::json!({"directory": vault})),
        // graph family
        tool_call(8, "get_graph_relationships", serde_json::json!({"path": note, "directory": vault})),
        // RAG/retrieval family
        tool_call(9, "chunk_markdown", serde_json::json!({"file_path": note})),
        tool_call(10, "retrieve_context", serde_json::json!({"query": "note", "directory": vault})),
        // text analytics family
        tool_call(11, "get_text_statistics", serde_json::json!({"file_path": note})),
        tool_call(12, "analyze_readability", serde_json::json!({"content": "Short sentence. Another one."})),
        tool_call(13, "count_tokens", serde_json::json!({"content": "hello world"})),
        // AI family: no API key in CI — must degrade to a setup note, not error
        tool_call(14, "ai_summarize", serde_json::json!({"content": "some text"})),
        // Obsidian family
        tool_call(15, "obsidian_vault_index", serde_json::json!({"vault_path": vault})),
        tool_call(16, "obsidian_search", serde_json::json!({"vault_path": vault, "query": "note", "mode": "text"})),
        // meta
        tool_call(17, "get_tool_help", serde_json::json!({})),
    ];
    let responses = call(&requests);
    assert_eq!(responses.len(), requests.len());
    for id in 1..=17 {
        expect_text(&responses, id);
    }
    // Spot-check content, not just non-emptiness.
    assert!(expect_text(&responses, 1).contains("```python"));
    assert!(expect_text(&responses, 17).contains("convert_file"));
}

#[test]
fn resources_and_prompts_answer_over_jsonrpc() {
    let requests = vec![
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "resources/list", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "prompts/list", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "prompts/get",
            "params": {"name": "summarize_note", "arguments": {"path": "Note A.md"}}}),
    ];
    // Run with --base-dir so resources/list has content.
    let mut child = Command::new(env!("CARGO_BIN_EXE_to_markdown_mcp"))
        .args(["--base-dir", &vault()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for r in &requests {
        writeln!(stdin, "{}", r).unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let responses: Vec<serde_json::Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(!responses[0]["result"]["resources"].as_array().unwrap().is_empty());
    assert_eq!(responses[1]["result"]["prompts"].as_array().unwrap().len(), 3);
    assert!(responses[2]["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap()
        .contains("Note A.md"));
}

#[test]
fn errors_are_consistent_jsonrpc_errors() {
    let requests = vec![
        // Unknown method → -32601
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "no/such/method", "params": {}}),
        // Unknown tool → error response, not a crash
        tool_call(2, "no_such_tool", serde_json::json!({})),
        // Missing required parameter → error mentioning the parameter
        tool_call(3, "convert_file", serde_json::json!({})),
        // Nonexistent file → error, server keeps serving
        tool_call(4, "convert_file", serde_json::json!({"file_path": "/no/such/file.xyz"})),
        // Server still alive after all those errors
        tool_call(5, "convert_text", serde_json::json!({"content": "still alive"})),
    ];
    let responses = call(&requests);
    assert_eq!(responses[0]["error"]["code"], -32601);
    // Unknown tool and missing parameter are the caller's fault → -32602.
    assert_eq!(responses[1]["error"]["code"], -32602);
    assert_eq!(responses[2]["error"]["code"], -32602);
    assert!(
        responses[2]["error"]["message"].as_str().unwrap().contains("file_path"),
        "missing-parameter error should name the parameter: {}",
        responses[2]["error"]
    );
    // Failed execution on valid params → -32603 with the real cause.
    assert_eq!(responses[3]["error"]["code"], -32603);
    assert_ne!(responses[3]["error"]["message"], "Internal error");
    assert!(responses[4].get("error").is_none());
}
