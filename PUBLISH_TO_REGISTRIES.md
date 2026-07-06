# How to Publish toMarkdownMCP to All Registries

Step-by-step, in order. Do this once per release worth announcing.

---

## 📋 Prerequisites Checklist

- [ ] All tests pass: `cargo test` (272 green)
- [ ] Project builds: `cargo build --release`
- [ ] Git repo is clean: `git status`
- [ ] A tagged GitHub Release exists with binaries ([RELEASE.md](RELEASE.md))
- [ ] Repo is public

## Step 1 — GitHub discovery (2 min)

- Add repo topics: `mcp`, `mcp-server`, `markdown`, `obsidian`, `rag`, `rust`
  (Repo page → ⚙️ next to About → Topics)
- Set the About description to the one-liner from [MCP_SUBMISSION_GUIDE.md](MCP_SUBMISSION_GUIDE.md)

## Step 2 — mcp.so (5 min)

1. Open https://mcp.so/submit
2. Paste content from [MCP_SO_SUBMISSION.md](MCP_SO_SUBMISSION.md)
3. Submit; curation usually takes a few days

## Step 3 — Docker Hub image (15 min)

Follow [DOCKER_HUB_QUICK_START.md](DOCKER_HUB_QUICK_START.md):

```bash
docker login
docker build -t YOURUSER/tomarkdownmcp:latest -t YOURUSER/tomarkdownmcp:0.1.0 .
docker push YOURUSER/tomarkdownmcp:latest && docker push YOURUSER/tomarkdownmcp:0.1.0
```

Fill the Docker Hub repository Overview from [DOCKER_HUB_SETUP.md](DOCKER_HUB_SETUP.md).

## Step 4 — Docker MCP Registry (10 min)

1. Fork https://github.com/docker/mcp-registry
2. Add a catalog entry using the metadata in `MCP_REGISTRY_CONFIG.json`
3. Open a PR; reference the public Docker Hub image

## Step 5 — Official MCP servers list (optional)

The modelcontextprotocol org maintains a community servers list (https://github.com/modelcontextprotocol/servers). Add a one-line entry under Community Servers via PR:

```markdown
- **[toMarkdownMCP](https://github.com/carlomagnoglobal/toMarkdownMCP)** — Convert anything to Markdown (docs, office, live web via Chromium, Obsidian vaults) + RAG and token analytics. 62 tools, Rust.
```

## Step 6 — Verify listings

- [ ] mcp.so page live and renders correctly
- [ ] `docker pull YOURUSER/tomarkdownmcp` works from a clean machine
- [ ] GitHub search for "mcp markdown" surfaces the repo

## Ongoing

On each release: push new Docker tags, `gh release` (automated by the tag workflow), and update registry descriptions only when the pitch changes.
