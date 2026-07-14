# Publishing toMarkdownMCP to MCP Registries

Complete guide for submitting to both **mcp.so** and the **Docker Hub MCP Registry**.

---

## Registry Overview

| Registry | URL | Best For | Type |
|----------|-----|----------|------|
| **mcp.so** | https://mcp.so/submit | General MCP discovery | Curated registry |
| **Docker Hub MCP** | https://hub.docker.com/mcp | Docker users | Docker-focused |
| **GitHub topic** | add `mcp` + `mcp-server` repo topics | GitHub search | Free discovery |

## Prerequisites

- [ ] Repo is **public** on GitHub
- [ ] A tagged release exists with binaries ([RELEASE.md](RELEASE.md))
- [ ] README clearly states what the server does + how to install
- [ ] `MCP_REGISTRY_CONFIG.json` and `MCP_TOOL_SCHEMA.json` are up to date

## Server metadata (copy-paste source of truth)

- **Name**: toMarkdownMCP
- **One-liner**: Convert anything to Markdown — 62 tools for document/web conversion, Chromium capture, Obsidian vaults, RAG, and text analytics.
- **Description**: Rust MCP server (JSON-RPC 2.0 stdio, zero MCP-framework dependencies) that converts code, HTML, PDF/DOCX/XLSX/PPTX, email, ebooks, feeds, markup, and live JS-rendered web pages (real Chromium, human-in-the-loop for logins/CAPTCHAs) into clean Markdown — plus an Obsidian vault suite (wikilinks, backlinks, canvas, dataview, templates), RAG chunking/retrieval, provider-aware token analytics, and optional Claude-backed generation. The same binary is a terminal Markdown viewer.
- **Categories/tags**: markdown, converter, documents, obsidian, rag, browser, knowledge-base, rust
- **Command**: `to_markdown_mcp` (no args) · **Transport**: stdio
- **License**: MIT · **Repo**: https://github.com/carlomagnoglobal/toMarkdownMCP

## mcp.so submission

1. Go to https://mcp.so/submit
2. Paste the GitHub repo URL; fill name/description from above
3. Submission copy prepared in [MCP_SO_SUBMISSION.md](MCP_SO_SUBMISSION.md)

## Docker Hub MCP Registry

1. Publish the image first ([DOCKER_HUB_SETUP.md](DOCKER_HUB_SETUP.md))
2. Follow the current process at https://hub.docker.com/mcp (Docker curates from public images + a PR to their catalog repo, https://github.com/docker/mcp-registry)
3. The catalog entry needs: image name, description above, and the stdio command — all in `MCP_REGISTRY_CONFIG.json`

## Step-by-step for both

See [PUBLISH_TO_REGISTRIES.md](PUBLISH_TO_REGISTRIES.md) for the full ordered checklist and [MCP_SUBMISSION_GUIDE.md](MCP_SUBMISSION_GUIDE.md) for form-field-by-form-field copy.
