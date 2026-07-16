# Multi-stage Dockerfile for toMarkdownMCP

# Stage 1: Builder
FROM rust:latest AS builder

WORKDIR /app

COPY Cargo.toml ./
COPY src ./src
COPY tests ./tests

# The workspace manifest lists the GUI crate as a member; a stub manifest and
# entry point satisfy cargo without pulling the Tauri toolchain into the image
# (default-members means only the MCP crate is actually built).
RUN mkdir -p gui/src \
    && printf '[package]\nname = "to_markdown_gui"\nversion = "0.0.0"\nedition = "2021"\npublish = false\n' > gui/Cargo.toml \
    && printf 'fn main() {}\n' > gui/src/main.rs

RUN cargo build --release --bin to_markdown_mcp

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

# ca-certificates: HTTPS fetching (convert_from_source, browser tools)
# chromium + fonts: the browser_* capture tools. Remove these two packages
# for a slim (~100MB) image if you don't need browser capture.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    chromium \
    fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/to_markdown_mcp /usr/local/bin/to_markdown_mcp

# Point the browser tools at the bundled Chromium.
ENV CHROME=/usr/bin/chromium

# Non-root user (Chromium refuses to sandbox as root anyway)
RUN useradd -m mcp
USER mcp

# MCP servers speak JSON-RPC over stdio — run with `docker run -i`.
ENTRYPOINT ["/usr/local/bin/to_markdown_mcp"]
