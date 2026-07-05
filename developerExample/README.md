# Developer Example: Browser Capture → Markdown → TUI Viewer

Reproducible recipe for testing the browser-tools + TUI pipeline end-to-end:
fetch a real URL through headless Chromium, convert it to Markdown, then view
it in the terminal UI — including how to capture the TUI's screen
non-interactively for automated testing.

## 1. Fetch a URL and convert it to Markdown (`browser_capture_markdown`)

Build the binary once, then drive it over JSON-RPC/stdio:

```bash
cd /Users/elisjmendez/Documents/toMarkdownMCP
cargo build
python3 developerExample/capture_url.py https://example.com/some-article developerExample/output.md
```

See [`capture_url.py`](capture_url.py). It opens headless Chromium, renders
the page (JS included), extracts YAML frontmatter metadata, and converts the
DOM to Markdown via the same pipeline `convert_from_source` uses.

## 2. View the result in the TUI

Interactively (recommended — just run it yourself):

```bash
./target/debug/to_markdown_mcp tui developerExample/output.md
```

Keys: `Tab` switch pane, `j`/`k` or arrows to scroll/move, `Enter` open/follow
`[[wikilink]]`, `/` search, `q` quit.

## 3. Capture the TUI screen non-interactively (for automated tests)

The TUI needs a real terminal (raw mode), so to render it without a human at
the keyboard, use a PTY plus a Python terminal emulator (`pyte`) that
interprets the ANSI output into plain text.

```bash
python3 -m venv /tmp/tui-test-venv
/tmp/tui-test-venv/bin/pip install --quiet pyte
/tmp/tui-test-venv/bin/python3 developerExample/capture_tui.py developerExample/output.md
```

See [`capture_tui.py`](capture_tui.py) for the full script.

### The one gotcha

Send keys **one at a time with a short delay between them**, not as a rapid
burst. A burst sent immediately after spawning the process can land in the
PTY's line-buffered ("cooked") mode before the app's `enable_raw_mode()` call
takes effect, so keystrokes appear to get silently dropped — that's a
test-harness race, not an app bug. Give it ~1–1.5s warm-up after spawn, then
pace keys ~100–150ms apart.

## Files

- [`capture_url.py`](capture_url.py) — calls `browser_capture_markdown` +
  `browser_close` over stdio JSON-RPC and saves the Markdown to a file.
- [`capture_tui.py`](capture_tui.py) — opens the TUI in a PTY, drives it with
  paced keypresses, and dumps a plain-text snapshot of the rendered screen.
