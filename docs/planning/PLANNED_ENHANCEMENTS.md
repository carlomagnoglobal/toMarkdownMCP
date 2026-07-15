# Planned Enhancements for toMarkdownMCP

Status ledger for the original HTML/web-conversion enhancement list. Almost everything here has shipped — see the linked feature docs. Remaining work and all new directions live in [ROADMAP.md](ROADMAP.md).

## Shipped ✅

| Enhancement | Feature doc |
|---|---|
| Full .webarchive (Safari) support | [WEBARCHIVE_SUPPORT.md](../features/WEBARCHIVE_SUPPORT.md) |
| Image extraction & embedding | [IMAGE_EXTRACTION.md](../features/IMAGE_EXTRACTION.md) |
| Metadata extraction (YAML frontmatter) | [METADATA_EXTRACTION.md](../features/METADATA_EXTRACTION.md) |
| CSS styling hints in Markdown | [CSS_STYLING_HINTS.md](../features/CSS_STYLING_HINTS.md) |
| Table of contents generation | [TOC_GENERATION.md](../features/TOC_GENERATION.md) |
| HTML table conversion | [TABLE_CONVERSION.md](../features/TABLE_CONVERSION.md) |
| Code block language auto-detection | [CODE_LANGUAGE_DETECTION.md](../features/CODE_LANGUAGE_DETECTION.md) |
| HTML form extraction | [FORM_EXTRACTION.md](../features/FORM_EXTRACTION.md) |
| HTML comments preservation | [COMMENT_PRESERVATION.md](../features/COMMENT_PRESERVATION.md) |

## Still planned

### Performance optimization for large files
**Complexity:** Medium-High

Streaming/chunked conversion path for HTML and documents larger than ~10MB, with lazy evaluation, to keep memory bounded. Tracked as part of the Hardening phase in [ROADMAP.md](ROADMAP.md).

### Interactive elements documentation
**Complexity:** Low-Medium

Convert buttons, dropdowns, and modals into text descriptions or structured documentation (similar in spirit to form extraction).

### SVG to ASCII art
**Complexity:** High, low priority

Convert embedded SVG graphics to ASCII art for terminal viewing. May be dropped in favor of SVG passthrough/rasterization once the GUI viewer exists.

## References

- [ROADMAP.md](ROADMAP.md) — current multi-phase development roadmap
- [HTML_SUPPORT.md](../features/HTML_SUPPORT.md) — HTML conversion documentation
- `src/html_converter.rs` — main HTML conversion module
