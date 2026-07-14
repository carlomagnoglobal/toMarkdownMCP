# MCP Submission Guide — Field-by-Field Copy

Ready-to-paste values for registry submission forms. Keep in sync with README/CHANGELOG.

## Identity

| Field | Value |
|---|---|
| Name | toMarkdownMCP |
| Author | Carlo Magno Global |
| Repository | https://github.com/carlomagnoglobal/toMarkdownMCP |
| License | MIT |
| Language | Rust |
| Transport | stdio (JSON-RPC 2.0, newline-delimited) |
| Command | `/path/to/to_markdown_mcp` (no arguments) |
| Docker | `docker run -i --rm YOURUSER/tomarkdownmcp` |

## Short description (≤140 chars)

> Convert anything to Markdown: docs, office files, live web pages (Chromium), Obsidian vaults, RAG chunking & token analytics. 62 tools.

## Long description

> toMarkdownMCP is a self-contained Rust MCP server exposing 62 tools. It converts 60+ programming languages, HTML/MHTML/webarchive, PDF, DOCX, XLSX, PPTX, RTF/ODT/ODS/ODP, EML, EPUB/MOBI, RSS/Atom, and MediaWiki/RST/AsciiDoc/Org/LaTeX/Textile markup to clean Markdown — from files, URLs, or stdin. A real Chromium integration captures JS-rendered pages and supports human-in-the-loop flows (log in or solve a CAPTCHA in a visible window, then capture). An 11-tool Obsidian suite understands wikilinks, aliases, backlinks, tasks, canvas files, Dataview fields, and templates. A RAG toolkit provides heading-aware chunking, budgeted retrieval with citations, dedup, clustering, and corpus statistics. `analyze_text` reports words/chars/spaces/tokens with sorted frequency tables and provider-aware tokenization (exact tiktoken for OpenAI; clearly-flagged estimates for Claude/Grok; HuggingFace tokenizer.json support for Llama/Qwen/DeepSeek). Optional Claude-backed summarize/ask/tag/translate/classify tools activate with ANTHROPIC_API_KEY. The same binary doubles as a rich terminal Markdown viewer.

## Example config (Claude Desktop)

```json
{
  "mcpServers": {
    "toMarkdown": {
      "command": "/path/to/toMarkdownMCP/target/release/to_markdown_mcp"
    }
  }
}
```

## Example prompts for screenshots

- "Convert https://example.com/article to Markdown and save it to notes/article.md"
- "Index my Obsidian vault at ~/vault and list broken links and orphan notes"
- "Analyze tokens in report.md for gpt-4o and show the top 20 words"

## Assets

- Tool schema: [`MCP_TOOL_SCHEMA.json`](../mcp_functions/MCP_TOOL_SCHEMA.json) (generated from the live binary)
- Registry config: `MCP_REGISTRY_CONFIG.json`
- Test evidence: [MCP_TEST_RESULTS.md](../planning/MCP_TEST_RESULTS.md)
