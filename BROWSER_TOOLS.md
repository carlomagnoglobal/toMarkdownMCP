# Browser Tools (Chromium-based Web Capture)

Three tools drive a real Chromium browser to fetch web pages that plain HTTP
fetching (`convert_from_source`) cannot handle: JavaScript-rendered content,
bot checks, cookie walls, CAPTCHAs, and login-gated pages.

The browser session is **shared across tool calls**, enabling a
human-in-the-loop workflow: open a visible window, interact with the page
yourself, then capture the result as Markdown.

## Requirements

A Chromium-based browser installed locally — Google Chrome, Chromium, or
Microsoft Edge. Auto-detected from standard locations (e.g.
`/Applications/Google Chrome.app` on macOS); override with the `CHROME`
environment variable pointing at the browser executable.

## Tools

### `browser_open_url`

Launch Chromium and navigate to a URL. Returns immediately after page load,
leaving the session open.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| url | string | yes | – | URL to open (http/https) |
| visible | boolean | no | false | Show the browser window for human interaction |
| wait_seconds | integer | no | 0 | Extra settle time after load for JS-heavy pages |
| timeout_seconds | integer | no | 30 | Navigation timeout |
| user_agent | string | no | – | Custom User-Agent string |

On navigation timeout the session **stays open** so you can retry or capture
the partial page. Calling `browser_open_url` again replaces the existing
session.

### `browser_capture_markdown`

Capture the current rendered DOM (`page.content()`) and convert it to
Markdown through the same HTML pipeline as `convert_from_source`.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| url | string | no | – | Navigate here first; opens a headless session if none exists |
| wait_seconds | integer | no | 0 | Extra settle time when `url` is given |
| timeout_seconds | integer | no | 30 | Navigation timeout when `url` is given |
| extract_metadata | boolean | no | false | YAML frontmatter from page metadata |
| preserve_css_hints | boolean | no | false | CSS hints as HTML comments |
| generate_toc | boolean | no | false | Table of contents from headings |
| toc_max_level | integer | no | 3 | Max TOC heading level (1–6) |
| extract_images | boolean | no | false | Process images |
| image_format | string | no | link | `link`, `reference`, or `base64` |
| convert_tables | boolean | no | false | HTML tables → Markdown tables |
| extract_forms | boolean | no | false | Extract form structure |
| preserve_comments | boolean | no | false | Preserve HTML comments |
| extract_links | boolean | no | false | Link summary |
| analyze_headings | boolean | no | false | Heading structure analysis |
| extract_definition_lists | boolean | no | false | Definition list extraction |

### `browser_close`

Close the browser session and free resources. No parameters.

## Workflows

**One-shot JS-rendered page (headless):**

```json
{"name": "browser_capture_markdown", "arguments": {"url": "https://spa-site.example", "wait_seconds": 2, "convert_tables": true}}
```

**Human-in-the-loop (CAPTCHA / login):**

1. `browser_open_url` with `{"url": "https://gated.example", "visible": true}` — a Chrome window opens.
2. You solve the CAPTCHA / log in / dismiss banners in that window.
3. `browser_capture_markdown` with `{}` — converts the page as it now stands.
4. `browser_close` when done.

Because `browser_open_url` returns as soon as the page loads, the MCP server
is never blocked while you interact with the window.

## Implementation notes

- Built on [`chromiumoxide`](https://crates.io/crates/chromiumoxide) (Chrome
  DevTools Protocol, tokio runtime) in `src/browser.rs`.
- One global session (`tokio::sync::Mutex<Option<BrowserSession>>`); the CDP
  event loop runs on a spawned tokio task for the browser's lifetime.
- HTML → Markdown conversion reuses `convert_html_with_flags` in
  `src/main.rs`, the same pipeline `convert_from_source` uses.
