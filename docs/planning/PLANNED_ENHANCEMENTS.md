# Planned Enhancements for toMarkdownMCP

Future features and improvements for HTML/web content conversion and Markdown generation.

## Priority: High

### 1. Full .webarchive (Safari) Support
**Status:** Planned  
**Complexity:** Medium  
**Dependencies:** `plist` crate

Convert Safari web archives (.webarchive format) used on macOS/iOS to Markdown. Currently only supports HTML/HTM/MHTML.

**Implementation Plan:**
- Add plist crate for parsing Apple plist format
- Create .webarchive extractor
- Extract HTML and embedded resources
- Integrate into conversion pipeline

---

### 2. Image Extraction & Embedding
**Status:** Planned  
**Complexity:** Medium-High  
**Dependencies:** None (optional: `image` crate for optimization)

Extract images from HTML documents and either embed as base64 in Markdown or link to them.

**Features:**
- Automatic image URL extraction from `<img>` tags
- Multiple output modes:
  - `embed`: Base64-encoded images in Markdown
  - `link`: External links to images
  - `skip`: Ignore images
- Configurable via `embed_images` and `image_format` parameters
- Handle relative and absolute URLs
- Auto-detect image types (PNG, JPEG, WebP, etc.)

**Example Output:**
```markdown
![Alt text](data:image/png;base64,iVBORw0KGgo...)
```

---

### 3. Metadata Extraction
**Status:** Planned  
**Complexity:** Medium  
**Dependencies:** None

Extract document metadata and generate YAML frontmatter for Markdown files.

**Metadata Sources:**
- Standard meta tags: `title`, `description`, `author`, `keywords`
- Open Graph: `og:title`, `og:description`, `og:image`, `og:type`
- Dublin Core: `dc:creator`, `dc:date`, `dc:subject`
- Custom: `<meta>` tags with various naming schemes

**Output Format:**
```markdown
---
title: Document Title
author: Author Name
description: Document description
date: 2024-06-22
keywords:
  - keyword1
  - keyword2
---

# Document Title
...
```

**Features:**
- `extract_metadata: true/false` parameter
- YAML frontmatter generation
- Automatic date parsing
- Fallback to sensible defaults

---

### 4. CSS Styling Hints in Markdown
**Status:** Planned  
**Complexity:** Low-Medium  
**Dependencies:** None

Preserve CSS styling intent through HTML comments and hints in Markdown output.

**Implementation:**
- Extract inline styles from elements
- Parse class-based styling if CSS is available
- Add semantic HTML comments with styling information
- Focus on meaningful styles (colors, emphasis, alignment)

**Example Output:**
```markdown
<!-- color: #ff0000; font-weight: bold -->
**Important Note**

<!-- text-align: center -->
This is centered text
```

**Features:**
- `preserve_css_hints: true/false` parameter
- Smart detection (ignore default styles)
- Common patterns support (colors, fonts, alignment, sizing)

---

### 5. Table of Contents Generation
**Status:** Planned  
**Complexity:** Low-Medium  
**Dependencies:** None

Automatically generate a table of contents from heading structure with internal links.

**Features:**
- Extract all headings from converted Markdown
- Create linked TOC at document start
- Configurable heading level limits
- Auto-generate anchor links
- Customize TOC title

**Example Output:**
```markdown
## Table of Contents
- [Getting Started](#getting-started)
  - [Installation](#installation)
  - [Configuration](#configuration)
- [Usage](#usage)
  - [Basic Usage](#basic-usage)
  - [Advanced Options](#advanced-options)

---

## Getting Started

### Installation
...
```

**Parameters:**
- `generate_toc: true/false`
- `toc_max_level: 2-4` (which heading levels to include)
- `toc_title: "string"` (customize TOC heading)

---

## Priority: Medium

### 6. HTML Table Conversion
**Status:** Planned  
**Complexity:** Medium

Convert HTML `<table>` elements to Markdown pipe tables or structured lists.

```html
<table>
  <tr><th>Name</th><th>Age</th></tr>
  <tr><td>Alice</td><td>30</td></tr>
  <tr><td>Bob</td><td>25</td></tr>
</table>
```

Converts to:
```markdown
| Name  | Age |
|-------|-----|
| Alice | 30  |
| Bob   | 25  |
```

---

### 7. Code Block Language Auto-Detection
**Status:** Planned  
**Complexity:** Medium

Detect programming language from HTML code blocks and add proper Markdown syntax highlighting.

**Detection Methods:**
- Parse `class="language-xyz"` attributes
- Analyze code content patterns (keywords, syntax)
- Check `data-language` attributes
- Use heuristics for language identification

---

### 8. HTML Form Extraction
**Status:** Planned  
**Complexity:** Medium

Document HTML forms in a readable Markdown format, preserving field information.

**Example Output:**
```markdown
### Contact Form
- **Name** (text, required)
- **Email** (email, required)
- **Message** (textarea, optional)
- **Subscribe** (checkbox)
- [Submit] [Cancel]
```

---

## Priority: Low

### 9. HTML Comments Preservation
Extract HTML comments and convert to Markdown format (blockquotes or HTML comments).

### 10. Interactive Elements Documentation
Convert buttons, dropdowns, modals into text descriptions or structured documentation.

### 11. SVG to ASCII Art
Convert embedded SVG graphics to ASCII art for terminal viewing.

### 12. Performance Optimization
Optimize for large HTML files (>10MB) with streaming processing and lazy evaluation.

---

## Implementation Roadmap

**Phase 1 (Next):**
1. Metadata Extraction (Medium complexity, high value)
2. CSS Styling Hints (Low complexity, useful)
3. Table of Contents (Low complexity, useful)

**Phase 2:**
4. Image Extraction (Medium-High, valuable)
5. HTML Table Conversion (Medium, common)
6. Full .webarchive Support (Medium, specific use case)

**Phase 3:**
7. Code Block Language Detection
8. HTML Form Extraction
9. Other features as needed

---

## Testing Strategy

For each enhancement:
- Add example files in `examples/` directory
- Unit tests in respective module
- Integration tests in `tests/` directory
- Update HTML_SUPPORT.md documentation
- Update README with new features

---

## Configuration Example

Potential configuration structure for future versions:

```json
{
  "html": {
    "metadata": {
      "extract": true,
      "format": "yaml-frontmatter"
    },
    "images": {
      "extract": true,
      "embed": true,
      "base64_threshold_bytes": 50000
    },
    "tables": {
      "convert": true,
      "format": "markdown-pipe"
    },
    "css": {
      "preserve_hints": true,
      "preserve_classes": false
    },
    "toc": {
      "generate": true,
      "max_level": 3,
      "title": "Table of Contents"
    },
    "code": {
      "detect_language": true,
      "highlight_syntax": true
    },
    "forms": {
      "extract": true,
      "format": "list"
    }
  }
}
```

---

## References

- **HTML_SUPPORT.md** - Current HTML conversion documentation
- **src/html_converter.rs** - Main HTML conversion module
- **examples/** - Test and example files
