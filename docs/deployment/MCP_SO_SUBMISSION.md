# mcp.so Submission

Copy for https://mcp.so/submit — paste-ready.

**Server URL**: `https://github.com/carlomagnoglobal/toMarkdownMCP`

**Name**: `toMarkdownMCP`

**Tagline**: Convert anything to Markdown — documents, office files, live web pages, Obsidian vaults — plus RAG and token analytics. 62 tools, pure Rust.

**Description**:

toMarkdownMCP is a fast, dependency-light MCP server written in Rust (stdio JSON-RPC, no MCP framework). It turns almost any content into clean Markdown and gives agents a full knowledge workbench:

- 📄 **Universal conversion** — 60+ languages, HTML/MHTML/webarchive, PDF, DOCX, XLSX, PPTX, RTF/ODT, EML, EPUB/MOBI, RSS/Atom, MediaWiki/RST/AsciiDoc/Org/LaTeX/Textile; from files, URLs, or stdin
- 🌐 **Real browser capture** — Chromium-driven fetching of JS-rendered pages with human-in-the-loop (open a visible window, log in / solve the CAPTCHA, capture)
- 🗂 **Obsidian vault suite** — wikilink/backlink graph, aliases, tasks, canvas → Markdown, Dataview fields, templates, link-safe renames
- 🧠 **RAG toolkit** — heading-aware chunking, budgeted retrieval with citations, dedup/clustering, knowledge index, corpus stats
- 🔢 **Token analytics** — words/chars/spaces/tokens + frequency tables with provider-aware tokenizers (exact OpenAI tiktoken, HF tokenizer.json for Llama/Qwen/DeepSeek, flagged estimates for Claude/Grok)
- 🤖 **Optional Claude tools** — summarize/ask/tag/translate/classify with ANTHROPIC_API_KEY
- 🖥 **Bonus**: the same binary is a beautiful terminal Markdown viewer (`to_markdown_mcp tui`)

**Category**: Developer Tools / Knowledge Management

**Tags**: `markdown` `converter` `obsidian` `rag` `browser` `documents` `rust`

**Install**:

```bash
git clone https://github.com/carlomagnoglobal/toMarkdownMCP && cd toMarkdownMCP && cargo build --release
```

Claude Desktop config:

```json
{"mcpServers": {"toMarkdown": {"command": "/path/to/toMarkdownMCP/target/release/to_markdown_mcp"}}}
```
