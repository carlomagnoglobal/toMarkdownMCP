#!/usr/bin/env python3
"""Fetch a URL through the MCP server's browser_capture_markdown tool and
save the resulting Markdown to a file.

Usage:
    python3 capture_url.py <url> [output.md]

Requires the server binary to be built first:
    cargo build
"""
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BINARY = REPO_ROOT / "target" / "debug" / "to_markdown_mcp"


def capture(url: str, wait_seconds: int = 2, extract_metadata: bool = True) -> str:
    requests = [
        {
            "jsonrpc": "2.0",
            "id": "1",
            "method": "tools/call",
            "params": {
                "name": "browser_capture_markdown",
                "arguments": {
                    "url": url,
                    "wait_seconds": wait_seconds,
                    "extract_metadata": extract_metadata,
                },
            },
        },
        {
            "jsonrpc": "2.0",
            "id": "2",
            "method": "tools/call",
            "params": {"name": "browser_close", "arguments": {}},
        },
    ]

    proc = subprocess.Popen(
        [str(BINARY)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    stdin_payload = "".join(json.dumps(r) + "\n" for r in requests)
    out, _ = proc.communicate(stdin_payload, timeout=90)

    for line in out.splitlines():
        response = json.loads(line)
        if response["id"] == "1":
            if response.get("error"):
                raise RuntimeError(f"browser_capture_markdown failed: {response['error']}")
            return response["result"]["content"][0]["text"]

    raise RuntimeError("No response for browser_capture_markdown call")


def main() -> None:
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    url = sys.argv[1]
    output_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("output.md")

    if not BINARY.exists():
        print(f"Binary not found at {BINARY} — run `cargo build` first.", file=sys.stderr)
        sys.exit(1)

    markdown = capture(url)
    output_path.write_text(markdown)
    print(f"Saved {len(markdown)} chars to {output_path}")


if __name__ == "__main__":
    main()
