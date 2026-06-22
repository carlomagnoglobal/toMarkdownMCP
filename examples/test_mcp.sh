#!/bin/bash
# Example script to test the toMarkdownMCP server

# Build the project first
cargo build --release

# Start the server in the background
./target/release/to_markdown_mcp &
SERVER_PID=$!

# Give the server a moment to start
sleep 0.5

# Test 1: List available tools
echo "=== Test 1: List Tools ==="
echo '{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "tools/list",
  "params": {}
}' | nc localhost 9999 || echo "Note: This requires the server to listen on a port"

# Test 2: Convert a file
echo -e "\n=== Test 2: Convert File ==="
echo '{
  "jsonrpc": "2.0",
  "id": "2",
  "method": "tools/call",
  "params": {
    "name": "convert_file",
    "arguments": {
      "file_path": "examples/test.py",
      "include_filename": true
    }
  }
}' | ./target/release/to_markdown_mcp

# Test 3: Convert text content
echo -e "\n=== Test 3: Convert Text ==="
echo '{
  "jsonrpc": "2.0",
  "id": "3",
  "method": "tools/call",
  "params": {
    "name": "convert_text",
    "arguments": {
      "content": "fn main() { println!(\"Hello, World!\"); }",
      "file_type": "rust",
      "title": "Rust Hello World"
    }
  }
}' | ./target/release/to_markdown_mcp

# Clean up
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo -e "\n=== Tests Complete ==="
