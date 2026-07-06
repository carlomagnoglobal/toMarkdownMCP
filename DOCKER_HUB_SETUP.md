# Docker Hub Setup — Full Guide

Publishing toMarkdownMCP images to Docker Hub, in depth. For the 5-minute version see [DOCKER_HUB_QUICK_START.md](DOCKER_HUB_QUICK_START.md).

## 1. Account & repository

1. Create/log into https://hub.docker.com
2. **Create Repository** → name `tomarkdownmcp`, public.
3. Fill the repository **Overview** (this becomes your registry listing):

```markdown
# toMarkdownMCP

Rust MCP server that converts anything to Markdown and adds a knowledge toolkit:
62 tools over JSON-RPC stdio — document/office/web conversion, Chromium browser
capture with human-in-the-loop, full Obsidian vault intelligence, RAG chunking &
retrieval, and provider-aware text/token analytics. Ships a terminal Markdown
viewer in the same binary.

## Usage
docker run -i --rm YOURUSER/tomarkdownmcp

## Claude Desktop
{"mcpServers":{"toMarkdown":{"command":"docker","args":["run","-i","--rm","YOURUSER/tomarkdownmcp"]}}}

Source: https://github.com/carlomagnoglobal/toMarkdownMCP (MIT)
```

## 2. Tagging strategy

| Tag | Meaning |
|---|---|
| `latest` | most recent release |
| `X.Y.Z` (e.g. `0.1.0`) | immutable, matches the git tag `vX.Y.Z` |

Build both on every release:

```bash
VERSION=0.1.0
docker build -t YOURUSER/tomarkdownmcp:latest -t YOURUSER/tomarkdownmcp:$VERSION .
docker push YOURUSER/tomarkdownmcp:latest && docker push YOURUSER/tomarkdownmcp:$VERSION
```

## 3. Image notes

- Multi-stage build: `rust:latest` builder → `debian:bookworm-slim` runtime.
- The runtime stage installs **chromium** + fonts so `browser_open_url` / `browser_capture_markdown` work headless inside the container (`CHROME=/usr/bin/chromium` is set). Delete those lines for a slim (~100MB) image without browser tools.
- Volumes: mount your documents with `-v /host/notes:/data` and pass container paths (`/data/...`) in tool arguments.
- `-i` is required — MCP runs over stdin/stdout.

## 4. Multi-arch (optional)

```bash
docker buildx create --use --name multi 2>/dev/null || docker buildx use multi
docker buildx build --platform linux/amd64,linux/arm64 \
  -t YOURUSER/tomarkdownmcp:latest -t YOURUSER/tomarkdownmcp:$VERSION --push .
```

## 5. Docker MCP Registry

Once the image is public on Docker Hub, submit it to the Docker MCP Registry — steps and copy in [MCP_REGISTRIES_GUIDE.md](MCP_REGISTRIES_GUIDE.md).
