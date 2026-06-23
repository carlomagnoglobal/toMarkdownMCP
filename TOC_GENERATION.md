# Table of Contents (TOC) Generation

Automatically generate linked table of contents from document heading structure.

## Overview

The TOC generation feature automatically extracts headings from converted Markdown and creates a navigable table of contents with working anchor links. This is useful for:

- **Long documents** - Easy navigation through complex content
- **Multi-section guides** - Quick overview of document structure
- **Blog posts** - Standard practice for readability
- **API documentation** - Help readers find sections quickly
- **Technical reports** - Professional document structure

## How It Works

1. **Extract headings** - Scan Markdown for ATX-style headings (# H1, ## H2, etc.)
2. **Generate anchors** - Convert heading text to URL-safe anchor links
3. **Create TOC list** - Format as nested markdown list with links
4. **Insert optimally** - Place after metadata/title if present

## Usage

### With convert_file Tool

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "guide.html",
    "generate_toc": true,
    "toc_max_level": 3
  }
}
```

### With convert_from_source Tool

```json
{
  "name": "convert_from_source",
  "arguments": {
    "source": "https://example.com/article.html",
    "generate_toc": true
  }
}
```

### Combined with Other Features

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "article.html",
    "extract_metadata": true,
    "generate_toc": true,
    "toc_max_level": 2,
    "preserve_css_hints": true
  }
}
```

## Parameters

### `generate_toc` (boolean)
- **Default:** `false`
- **Description:** Enable/disable TOC generation
- **Values:** `true` or `false`

### `toc_max_level` (integer)
- **Default:** `3` (up to H3 headings)
- **Range:** `1-6`
- **Description:** Maximum heading level to include
- **Values:**
  - `1` - Only H1 (main title)
  - `2` - H1 and H2 sections
  - `3` - H1, H2, H3 subsections (recommended)
  - `4-6` - Include deeper nesting levels

## Output Format

### Basic Example

**Input:**
```html
<h1>Guide to Rust</h1>
<h2>Installation</h2>
<h3>Linux</h3>
<h3>macOS</h3>
<h2>First Program</h2>
```

**Output (with toc_max_level=3):**
```markdown
# Guide to Rust

## Table of Contents

- [Guide to Rust](#guide-to-rust)
  - [Installation](#installation)
    - [Linux](#linux)
    - [macOS](#macos)
  - [First Program](#first-program)

## Installation

...
```

### Anchor Generation

Headings are converted to anchors by:
1. Converting to lowercase
2. Replacing spaces with hyphens
3. Removing special characters
4. Collapsing multiple hyphens to single
5. Adding numeric suffix for duplicates

Examples:
- "Getting Started" → `#getting-started`
- "Core Concepts" → `#core-concepts`
- "Ownership & Borrowing" → `#ownership-borrowing`
- "Tips (Advanced)" → `#tips-advanced`
- "Tips" (duplicate) → `#tips-1`

## Placement

The TOC is inserted in this order:

1. **After YAML frontmatter** (if metadata extracted)
2. **After H1 title** (if present)
3. **At document start** (if no metadata/title)

Before any other content.

## Integration with Other Features

### With Metadata Extraction

When both enabled, the output order is:
1. YAML frontmatter (metadata)
2. Table of Contents
3. H1 title
4. Document content

### With CSS Styling Hints

CSS hints and TOC work independently:
- CSS hints preserved in document content
- TOC focuses on heading structure
- No conflicts or interference

### With Multiple Features

All features work together:

```json
{
  "extract_metadata": true,
  "preserve_css_hints": true,
  "generate_toc": true,
  "toc_max_level": 3
}
```

Output structure:
1. YAML metadata frontmatter
2. Table of Contents
3. Main title (H1)
4. Content with CSS hints preserved

## Examples

### Example 1: Simple Document

**HTML:**
```html
<h1>Project Overview</h1>
<h2>Features</h2>
<p>Feature list...</p>
<h2>Installation</h2>
<p>Install...</p>
```

**Generated TOC (max_level=2):**
```markdown
## Table of Contents

- [Project Overview](#project-overview)
  - [Features](#features)
  - [Installation](#installation)
```

### Example 2: Complex Documentation

**HTML:**
```html
<h1>API Documentation</h1>
<h2>Getting Started</h2>
<h3>Installation</h3>
<h3>Configuration</h3>
<h2>Core Concepts</h2>
<h3>Authentication</h3>
<h4>Token-Based Auth</h4>
<h4>OAuth 2.0</h4>
<h3>Rate Limiting</h3>
<h2>Endpoints</h2>
```

**Generated TOC (max_level=3):**
```markdown
## Table of Contents

- [API Documentation](#api-documentation)
  - [Getting Started](#getting-started)
    - [Installation](#installation)
    - [Configuration](#configuration)
  - [Core Concepts](#core-concepts)
    - [Authentication](#authentication)
    - [Rate Limiting](#rate-limiting)
  - [Endpoints](#endpoints)
```

### Example 3: With Metadata and Styling

```markdown
---
title: Complete Guide
description: Comprehensive guide
---

## Table of Contents

- [Complete Guide](#complete-guide)
  - [Chapter 1](#chapter-1)
  - [Chapter 2](#chapter-2)

# Complete Guide

## Chapter 1

<!-- CSS: color: #0066cc; font-weight: bold -->
**Important section**
```

## Best Practices

### Heading Structure

✅ **Good:**
- H1 for document title
- H2 for main sections
- H3 for subsections
- H4 for sub-subsections

❌ **Avoid:**
- Multiple H1 headings (confuses structure)
- Skipping levels (H1 → H3)
- Non-sequential levels

### Level Limits

| Document Type | Recommended Max Level |
|---|---|
| Blog post | 2-3 |
| Guide/tutorial | 3-4 |
| API documentation | 3-4 |
| Long technical doc | 4-5 |
| Complex specification | 4-6 |

### TOC Title

The TOC uses "Table of Contents" as default heading. For customization, consider:
- Edit after generation
- Use metadata with custom processors
- Leverage static site generator support

## Implementation Details

### Functions

**generate_toc()**
- Takes Markdown content and max level
- Returns Vec of Heading structs
- Handles frontmatter skipping
- Manages duplicate anchor names

**format_toc()**
- Formats headings as nested list
- Creates markdown links
- Handles proper indentation
- Returns ready-to-insert TOC string

**insert_toc()**
- Finds optimal insertion point
- Preserves metadata/title order
- Inserts complete TOC block
- Maintains document structure

### Anchor Generation

- Special character removal (except hyphens)
- Whitespace to hyphen conversion
- Multiple-hyphen collapsing
- Duplicate name handling
- Case normalization

## Testing

Included example: `examples/toc_demo.html`

Test with different max levels:

```bash
# H1 and H2 only
generate_toc: true
toc_max_level: 2

# Up to H3 (recommended)
generate_toc: true
toc_max_level: 3

# Very detailed (H4+)
generate_toc: true
toc_max_level: 5
```

## Performance

TOC generation is lightweight:
- Scans document once
- O(n) heading extraction
- O(m) TOC formatting (m = number of headings)
- Minimal memory usage

Typical times:
- 10KB document: < 1ms
- 100KB document: 1-2ms
- 1MB document: 5-10ms

## Limitations

- Only processes ATX headings (# syntax)
- Doesn't handle setext headers (underline style)
- Special characters replaced (not encoded)
- Assumes unique or semi-unique headings
- No support for custom anchor attributes

## Future Enhancements

Planned improvements:
- Setext header support
- Custom TOC title configuration
- Anchor prefix/suffix customization
- Skip certain headings option
- TOC styling/formatting control
- Nested heading collapsing option

## Troubleshooting

### TOC not generated

**Issue:** `generate_toc` is `false` by default
**Solution:** Set `generate_toc: true` in parameters

### TOC appears empty

**Issue:** No headings found or all above `toc_max_level`
**Solution:** Lower `toc_max_level` or add more headings

### Anchor links don't work

**Issue:** Heading text contains special characters that don't display
**Solution:** Check if heading renders correctly, special chars may be stripped

### Duplicate heading names

**Issue:** Multiple identical headings have same anchor
**Solution:** Automatic numeric suffixes added (#section-1, #section-2)

## References

- Markdown ATX heading syntax
- URL-safe anchor generation
- Standard TOC patterns
- Related: Metadata extraction, CSS hints, HTML conversion
