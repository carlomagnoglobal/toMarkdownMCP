# toMarkdownMCP Deployment Summary

**Date:** 2026-07-16 (updated)
**Status:** ✅ v0.2.0 released — crates.io, GitHub binaries + desktop-app dmg, Docker Hub

---

## What Has Been Set Up

### 1. Pre-built Binary Distribution
- ✅ GitHub Actions release workflow (`.github/workflows/release.yml`) builds on every `v*` tag:
  - macOS Apple Silicon (aarch64)
  - macOS Intel (x86_64)
  - Linux (x86_64)
  - Windows (x86_64)
- ✅ Assets attach automatically to the GitHub Release
- ✅ `install.sh` downloads the right asset per platform, falls back to cargo build

### 2. Build from Source
- ✅ Plain `cargo build --release`; Rust 1.75+, no system deps beyond a C compiler
- ✅ CI workflow (`.github/workflows/ci.yml`) builds + tests every push/PR (272 tests)

### 3. Docker
- ✅ Multi-stage `Dockerfile` (rust builder → debian slim runtime)
- ✅ Chromium bundled in the runtime image so browser tools work in-container (`CHROME` preset)
- ✅ Docker Hub publication automated on `v*` tags (plus manual dispatch with a version input); multi-arch amd64/arm64

## Release state

| Item | Status |
|---|---|
| Version | 0.1.0 (Cargo.toml + CHANGELOG.md) |
| Git tag `v0.1.0` | ✅ |
| GitHub Release with notes + macOS arm64 binary | ✅ |
| CI + release workflows | ✅ committed |
| Cargo package metadata (description/license/repository) | ✅ |
| Registry submissions | ⬜ pending repo going public — [REGISTRY_SUMMARY.md](REGISTRY_SUMMARY.md) |

## Remaining manual steps (owner)

1. **Make the repo public**: GitHub → Settings → General → Change visibility
2. Add repo topics + About text ([PUBLISH_TO_REGISTRIES.md](PUBLISH_TO_REGISTRIES.md) Step 1)
3. Submit to mcp.so; publish the Docker image when desired
