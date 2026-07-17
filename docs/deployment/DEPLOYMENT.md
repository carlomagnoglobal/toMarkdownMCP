# toMarkdownMCP Deployment Guide

This document describes three ways to deploy and use the toMarkdownMCP MCP server.

---

## Option 1: Pre-built Binaries

Download pre-built binaries from [GitHub Releases](https://github.com/carlomagnoglobal/toMarkdownMCP/releases).

| Platform | Asset |
|---|---|
| macOS Apple Silicon | `to_markdown_mcp-vX.Y.Z-macos-arm64.tar.gz` |
| macOS Intel | `to_markdown_mcp-vX.Y.Z-macos-x86_64.tar.gz` |
| Linux x86_64 | `to_markdown_mcp-vX.Y.Z-linux-x86_64.tar.gz` |
| Windows x86_64 | `to_markdown_mcp-vX.Y.Z-windows-x86_64.zip` |

```bash
tar -xzf to_markdown_mcp-*.tar.gz
chmod +x to_markdown_mcp
./to_markdown_mcp --help
```

Or let the install script pick the right asset:

```bash
curl -fsSL https://raw.githubusercontent.com/carlomagnoglobal/toMarkdownMCP/main/install.sh | bash
```

macOS Gatekeeper note: unsigned binaries may need `xattr -d com.apple.quarantine ./to_markdown_mcp` on first run.

---

## Option 2: Build from Source

```bash
git clone https://github.com/carlomagnoglobal/toMarkdownMCP.git
cd toMarkdownMCP
cargo build --release          # binary: target/release/to_markdown_mcp
cargo test                     # 272 tests
```

Requirements: Rust 1.88+, a C compiler (for the oniguruma regex backend used by the HuggingFace tokenizer support). Browser tools additionally want Chrome/Chromium at runtime.

Cross-compile targets work as usual (`cargo build --release --target x86_64-unknown-linux-gnu`, etc.) — the release workflow builds all four platforms automatically on each tag.

---

## Option 3: Docker

A multi-stage `Dockerfile` ships in the repo. The runtime image includes **Chromium**, so the browser-capture tools work inside the container (this makes the image notably larger; remove the chromium install line if you don't need `browser_*` tools).

```bash
# Build
docker build -t tomarkdownmcp .

# Run as an MCP stdio server (for clients that spawn docker)
docker run -i --rm tomarkdownmcp
```

Claude Desktop config using Docker:

```json
{
  "mcpServers": {
    "toMarkdown": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "-v", "/path/to/your/notes:/data", "tomarkdownmcp"]
    }
  }
}
```

Mount the directories you want the tools to read/write (`-v host:/data`) and reference them by the container path in tool calls.

Publishing to Docker Hub: see [DOCKER_HUB_QUICK_START.md](DOCKER_HUB_QUICK_START.md) and [DOCKER_HUB_SETUP.md](DOCKER_HUB_SETUP.md).

---

## Which option should I pick?

| You are… | Use |
|---|---|
| Just trying it out on your Mac/PC | Option 1 (pre-built) |
| Developing/customizing, or on an unusual platform | Option 2 (source) |
| Deploying on a server / want isolation / Docker MCP Registry | Option 3 (Docker) |

## Release engineering

Cutting a new version (maintainers): see [RELEASE.md](RELEASE.md). CI (`.github/workflows/ci.yml`) builds and tests every push; the release workflow (`.github/workflows/release.yml`) builds and attaches all platform binaries whenever a `v*` tag is pushed.
