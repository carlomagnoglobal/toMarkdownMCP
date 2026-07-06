#!/bin/bash
# Installation script for toMarkdownMCP
# Downloads the latest pre-built release binary for your platform,
# falling back to a cargo source build.

set -e

REPO="carlomagnoglobal/toMarkdownMCP"
GITHUB_API="https://api.github.com/repos/$REPO"
BINARY="to_markdown_mcp"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info()  { printf "${GREEN}==>${NC} %s\n" "$1"; }
warn()  { printf "${YELLOW}==>${NC} %s\n" "$1"; }
fail()  { printf "${RED}error:${NC} %s\n" "$1"; exit 1; }

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
    Darwin)
        case "$ARCH" in
            arm64)  ASSET="macos-arm64" ;;
            x86_64) ASSET="macos-x86_64" ;;
            *) fail "Unsupported macOS architecture: $ARCH" ;;
        esac
        ;;
    Linux)
        case "$ARCH" in
            x86_64) ASSET="linux-x86_64" ;;
            *) warn "No pre-built binary for Linux/$ARCH — building from source"; ASSET="" ;;
        esac
        ;;
    *)
        warn "No pre-built binary for $OS — building from source"
        ASSET=""
        ;;
esac

download_release() {
    info "Looking up latest release of $REPO ..."
    URL=$(curl -fsSL "$GITHUB_API/releases/latest" \
        | grep -o "\"browser_download_url\": *\"[^\"]*${ASSET}[^\"]*\"" \
        | head -1 | sed 's/.*"\(https[^"]*\)"/\1/')
    [ -n "$URL" ] || return 1

    info "Downloading $URL"
    TMP=$(mktemp -d)
    trap 'rm -rf "$TMP"' EXIT
    curl -fsSL "$URL" -o "$TMP/pkg.tar.gz"
    tar -xzf "$TMP/pkg.tar.gz" -C "$TMP"

    mkdir -p "$INSTALL_DIR"
    install -m 755 "$TMP/$BINARY" "$INSTALL_DIR/$BINARY"

    # macOS quarantine flag on downloaded binaries
    if [ "$OS" = "Darwin" ] && command -v xattr >/dev/null; then
        xattr -d com.apple.quarantine "$INSTALL_DIR/$BINARY" 2>/dev/null || true
    fi
    return 0
}

build_from_source() {
    command -v cargo >/dev/null || fail "cargo not found — install Rust from https://rustup.rs first"
    info "Building from source (this takes a few minutes) ..."
    TMP=$(mktemp -d)
    git clone --depth 1 "https://github.com/$REPO.git" "$TMP/src"
    (cd "$TMP/src" && cargo build --release)
    mkdir -p "$INSTALL_DIR"
    install -m 755 "$TMP/src/target/release/$BINARY" "$INSTALL_DIR/$BINARY"
    rm -rf "$TMP"
}

if [ -n "$ASSET" ] && download_release; then
    info "Installed pre-built binary."
else
    [ -n "$ASSET" ] && warn "No release asset found — falling back to source build."
    build_from_source
fi

info "Installed: $INSTALL_DIR/$BINARY"
"$INSTALL_DIR/$BINARY" --help | head -3 || true

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) warn "$INSTALL_DIR is not on your PATH — add: export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac

info "Next: register with your MCP client — see INSTALL.md"
info "  claude mcp add toMarkdown -- $INSTALL_DIR/$BINARY"
