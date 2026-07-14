# Release Guide

How to cut and publish a toMarkdownMCP release.

## 0. One-time: making the repo public

GitHub → repo **Settings → General → Danger Zone → Change visibility → Public**. Before flipping:
- [ ] `git grep -iE "api[_-]?key|secret|token"` over tracked files comes back clean
- [ ] LICENSE present (MIT) and Cargo.toml carries `license`/`repository` metadata
- [ ] README renders correctly on GitHub (doc links resolve)

## 1. Prepare the version

```bash
# 1. Bump the version in Cargo.toml ([package] version)
# 2. Add a CHANGELOG.md section for the new version
# 3. Verify
cargo test                      # must be green
cargo build --release
printf '%s\n' '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}' \
  | ./target/release/to_markdown_mcp | python3 -c "import json,sys; print(len(json.load(sys.stdin)['result']['tools']),'tools')"
```

If the tool count changed, update the counts quoted in README.md, USAGE.md, and `get_tool_help` (src/main.rs), and regenerate `MCP_TOOL_SCHEMA.json`:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}' \
  | ./target/release/to_markdown_mcp \
  | python3 -c "import json,sys; json.dump({'tools': json.load(sys.stdin)['result']['tools']}, open('MCP_TOOL_SCHEMA.json','w'), indent=2)"
```

## 2. Commit, tag, push

```bash
git add Cargo.toml CHANGELOG.md MCP_TOOL_SCHEMA.json   # explicit paths
git commit -m "Release vX.Y.Z"
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin main vX.Y.Z
```

Pushing the tag triggers `.github/workflows/release.yml`, which builds release binaries for **macOS arm64, macOS x86_64, Linux x86_64, and Windows x86_64** and attaches them to the GitHub Release automatically.

## 3. Create the GitHub Release

```bash
gh release create vX.Y.Z --title "vX.Y.Z" --notes-file <(sed -n '/^## vX.Y.Z/,/^## v/p' CHANGELOG.md | sed '$d')
```

(Or `gh release create vX.Y.Z --generate-notes` for auto-notes.) If the workflow created a draft/release already, just edit it: `gh release edit vX.Y.Z --notes-file ...`.

You can attach a locally built binary immediately without waiting for CI:

```bash
cd target/release && tar -czf to_markdown_mcp-vX.Y.Z-macos-arm64.tar.gz to_markdown_mcp && cd -
gh release upload vX.Y.Z target/release/to_markdown_mcp-vX.Y.Z-macos-arm64.tar.gz
```

## 4. Verify

```bash
gh release view vX.Y.Z                 # notes + assets
gh run list --workflow=release.yml     # platform builds status
bash install.sh                        # end-to-end: downloads the new release
```

## 5. Announce / registries

For mcp.so and Docker Hub MCP Registry submissions, follow [PUBLISH_TO_REGISTRIES.md](PUBLISH_TO_REGISTRIES.md).

## Versioning policy

Semantic-ish: bump **minor** for new tools/features, **patch** for fixes. Tool count or breaking schema changes always get at least a minor bump and a CHANGELOG entry.
