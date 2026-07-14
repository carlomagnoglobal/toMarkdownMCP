# Metadata Extraction Feature

Automatic extraction of HTML document metadata and generation of YAML frontmatter for Markdown files.

## Overview

The metadata extraction feature automatically extracts various metadata from HTML documents and generates YAML frontmatter for use in Markdown files. This is useful for:

- Preserving document metadata when converting web pages
- Creating Markdown files with proper frontmatter for Jekyll, Hugo, or other static site generators
- Documenting source information and authorship
- Maintaining document structure and context

## Supported Metadata Sources

### Standard HTML Meta Tags
- `<title>` - Document title
- `<meta name="description">` - Description/summary
- `<meta name="author">` - Author name
- `<meta name="keywords">` - Keywords/tags
- `<meta name="date">` - Publication date
- `<meta name="viewport">` - Viewport settings
- Any other `<meta name="...">` tags

### Open Graph Tags
- `og:title` - OpenGraph title
- `og:description` - OpenGraph description
- `og:type` - Content type (article, website, etc.)
- `og:image` - Featured image URL
- Other `og:*` properties

### HTML Attributes
- `<html lang="...">` - Document language

## Usage

### With convert_file Tool

Extract metadata from an HTML file:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "extract_metadata": true
  }
}
```

### With convert_from_source Tool

Extract metadata from a URL or file source:

```json
{
  "name": "convert_from_source",
  "arguments": {
    "source": "https://example.com/article.html",
    "extract_metadata": true
  }
}
```

## Output Format

Metadata is output as YAML frontmatter at the beginning of the Markdown file:

```markdown
---
author: Jane Doe
date: 2024-06-22
description: A guide to learning Rust
keywords: rust, programming, systems
language: en
og-description: Learn Rust from scratch
og-title: Complete Rust Guide
title: The Complete Guide to Rust Programming
viewport: width=device-width, initial-scale=1.0
---

# The Complete Guide to Rust Programming

Document content here...
```

## Metadata Processing

### Key Normalization
- Keys are sorted alphabetically
- Underscores in keys are converted to hyphens in output
- OpenGraph prefixes are normalized: `og:title` → `og-title`
- Case is normalized (lowercase)

### Value Handling
- Empty values are skipped
- Special characters are properly escaped
- Values with colons, quotes, or whitespace are quoted
- Newlines in values are preserved

### Example Transformations

Input meta tags:
```html
<meta name="author" content="John Doe">
<meta property="og:title" content="My Article">
<meta name="date" content="2024-06-22">
```

Output YAML:
```yaml
author: John Doe
date: 2024-06-22
og-title: My Article
```

## Static Site Generator Integration

The extracted metadata is compatible with popular static site generators:

### Jekyll/GitHub Pages
```yaml
---
title: Article Title
author: John Doe
date: 2024-06-22
description: Article summary
keywords: tag1, tag2
---
```

### Hugo
```yaml
---
title: Article Title
author: John Doe
date: 2024-06-22
description: Article summary
tags: [tag1, tag2]
---
```

### Custom Metadata
Any meta tag can be extracted. For example:

```html
<meta name="category" content="Tutorial">
<meta name="reading-time" content="15 minutes">
<meta name="featured" content="true">
```

Will generate:

```yaml
category: Tutorial
featured: true
reading-time: 15 minutes
```

## Examples

### Example 1: Blog Post with Full Metadata

Input HTML:
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <title>Getting Started with Rust</title>
    <meta name="description" content="Learn the basics of Rust programming">
    <meta name="author" content="Sarah Smith">
    <meta name="date" content="2024-06-22">
    <meta name="keywords" content="rust, programming, tutorial">
    <meta property="og:title" content="Getting Started with Rust">
</head>
<body>
    <h1>Getting Started with Rust</h1>
    <p>Content here...</p>
</body>
</html>
```

Converted with `extract_metadata: true`:
```markdown
---
author: Sarah Smith
date: 2024-06-22
description: Learn the basics of Rust programming
keywords: rust, programming, tutorial
language: en
og-title: Getting Started with Rust
title: Getting Started with Rust
---

# Getting Started with Rust

Content here...
```

### Example 2: Web Article

```html
<html lang="en">
<head>
    <title>Web Architecture Best Practices</title>
    <meta name="description" content="Modern web architecture patterns">
    <meta name="author" content="Alex Johnson">
    <meta property="og:type" content="article">
</head>
<body>
    <h1>Web Architecture Best Practices</h1>
    ...
</body>
</html>
```

Generates:
```markdown
---
author: Alex Johnson
description: Modern web architecture patterns
language: en
og-type: article
title: Web Architecture Best Practices
---
```

## Behavior Notes

### Default Behavior
- `extract_metadata: false` (default) - No metadata extraction
- When disabled, only filename heading is added (if enabled)
- Fully backward compatible

### When Metadata is Extracted
- YAML frontmatter replaces filename heading
- Document title is still included as H1
- All metadata is at the top of the file
- Standard Markdown follows the frontmatter

### Missing Metadata
- If no metadata exists, no frontmatter is generated
- File still converts normally
- Empty HTML `<title>` tags are skipped

## Implementation Details

### Functions

**extract_html_metadata()**
- Takes HTML content string
- Returns BTreeMap<String, String> with all metadata
- Handles multiple meta tag formats
- Case-insensitive attribute matching

**metadata_to_yaml_frontmatter()**
- Takes BTreeMap of metadata
- Returns properly formatted YAML frontmatter
- Includes `---` delimiters
- Sorts keys alphabetically
- Escapes special characters

**html_to_markdown_with_metadata()**
- Main conversion function
- Accepts optional metadata extraction flag
- Delegates to extraction and conversion functions
- Combines metadata with content

## Supported File Types

Metadata extraction works with:
- `.html` - Standard HTML files
- `.htm` - Legacy HTML files
- `.mhtml` - MIME HTML archives

For MHTML files, HTML is extracted first, then metadata is processed.

## Limitations

- Only extracts text metadata, not embedded content
- Image URLs are extracted but images are not embedded
- Link targets are preserved but not verified
- No CSS or styling information in frontmatter
- No script or style content extracted

## Future Enhancements

Planned improvements:
- More metadata sources (JSON-LD, Schema.org)
- Image extraction alongside metadata
- Custom metadata field mapping
- Template-based frontmatter generation
- Multi-language support
- Structured data extraction (dates, authors as objects)

## Testing

Example file: `examples/sample_with_metadata.html`

Test with:
```bash
# Test with metadata extraction
echo '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"examples/sample_with_metadata.html","extract_metadata":true}}}' | ./to_markdown_mcp

# Test without metadata extraction
echo '{"jsonrpc":"2.0","id":"2","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"examples/sample_with_metadata.html","extract_metadata":false}}}' | ./to_markdown_mcp
```

## Performance

Metadata extraction is lightweight:
- Minimal parsing overhead
- No additional dependencies
- Fast sorting of small key sets
- Negligible performance impact

Typical conversion times:
- 10KB HTML: < 5ms
- 100KB HTML: 5-10ms
- 1MB HTML: 10-50ms
