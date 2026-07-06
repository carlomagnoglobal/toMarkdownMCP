# Docker Hub Quick Setup — 5 Minutes

Fast checklist to get toMarkdownMCP publishing to Docker Hub + the Docker MCP Registry.

---

## ✅ Checklist: 5-Minute Setup

### 1. Create a Docker Hub account (2 minutes)
- [ ] https://hub.docker.com/signup → sign up → verify email
- [ ] Note your username (referred to as `YOURUSER` below)

### 2. Create the repository (1 minute)
- [ ] Docker Hub → **Create Repository**
- [ ] Name: `tomarkdownmcp` · Visibility: Public
- [ ] Description: *"MCP server converting anything to Markdown — 62 tools: web capture, Obsidian vaults, RAG, text analytics"*

### 3. Build & push (2 minutes + build time)

```bash
cd /path/to/toMarkdownMCP
docker login
docker build -t YOURUSER/tomarkdownmcp:latest -t YOURUSER/tomarkdownmcp:0.1.0 .
docker push YOURUSER/tomarkdownmcp:latest
docker push YOURUSER/tomarkdownmcp:0.1.0
```

Note: the image includes Chromium so `browser_*` tools work; expect a multi-hundred-MB image. Strip the chromium line from the Dockerfile for a slim build.

### 4. Smoke test

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}' | docker run -i --rm YOURUSER/tomarkdownmcp | head -c 200
```

---

Full details (overview page content, tags strategy, MCP registry linkage): [DOCKER_HUB_SETUP.md](DOCKER_HUB_SETUP.md). Registry submissions: [PUBLISH_TO_REGISTRIES.md](PUBLISH_TO_REGISTRIES.md).
