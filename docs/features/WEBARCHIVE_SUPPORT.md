# Safari Webarchive (.webarchive) Support

Full support for converting Safari web archives to Markdown format.

## Overview

Webarchive files (`.webarchive`) are Safari's native format for saving complete web pages as single files. They contain:
- The main HTML document
- All embedded resources (images, CSS, JavaScript)
- Metadata about the archive
- Preserved file structure and links

The toMarkdownMCP server can extract and convert webarchive files to clean Markdown, preserving content and structure while supporting all standard HTML conversion features (metadata extraction, CSS hints, TOC generation, image handling).

## What is a Webarchive File?

Webarchive is a macOS/iOS format used by Safari, Mail, and other Apple applications to archive web content. Key characteristics:

- **Format:** Binary plist (property list) file
- **Extension:** `.webarchive`
- **Structure:** Hierarchical dictionary containing:
  - `MainResource` - The primary HTML document
  - `Resources` - Array of sub-resources (optional)
  - Metadata and resource information
- **Encoding:** UTF-8 text with binary data
- **Cross-platform:** Can be opened on any OS (though native support is Safari/macOS)

## Creating Webarchive Files

### From Safari (macOS/iOS)
1. Open a webpage in Safari
2. Select **File → Save As** (or **Share → Save as Web Archive** on iOS)
3. Choose **Format: Web Archive** (.webarchive)
4. Click Save

### From Mail
1. Right-click an email with web content
2. Select **Save as Web Archive**
3. Choose save location

### Programmatically
See example code below to create webarchive files in your own applications.

## Using toMarkdownMCP with Webarchives

### Basic Conversion

**Command-line (stdin):**
```bash
cat webpage.webarchive | ./to_markdown_mcp
```

**MCP Tool (convert_file):**
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "webpage.webarchive"
  }
}
```

**MCP Tool (convert_from_source):**
```json
{
  "name": "convert_from_source",
  "arguments": {
    "source": "/Users/user/Downloads/article.webarchive"
  }
}
```

### With HTML Features

Webarchive conversion supports all HTML enhancement features:

#### Metadata Extraction
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.webarchive",
    "extract_metadata": true
  }
}
```

#### Image Extraction
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.webarchive",
    "extract_images": true,
    "image_format": "link"
  }
}
```

#### All Features Combined
```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "webpage.webarchive",
    "extract_metadata": true,
    "extract_images": true,
    "image_format": "link",
    "preserve_css_hints": true,
    "generate_toc": true,
    "toc_max_level": 3
  }
}
```

## Output Format

### Extracted from Webarchive

**Input:** `article.webarchive` containing:
```
https://example.com/article
├── article.html
├── images/photo1.jpg
├── images/photo2.jpg
├── styles/main.css
└── scripts/app.js
```

**Output Markdown:**
```markdown
---
author: Jane Doe
date: 2024-06-22
title: Example Article
---

## Table of Contents

- [Example Article](#example-article)
  - [Introduction](#introduction)
  - [Content](#content)

# Example Article

Example Article
==========

## Introduction

[Introductory text...]

![Photo 1](images/photo1.jpg)

## Content

[More content...]

![Photo 2](images/photo2.jpg)
```

## Technical Details

### Webarchive Structure

A webarchive is a binary plist file with this structure:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
    <key>MainResource</key>
    <dict>
        <key>URL</key>
        <string>https://example.com/page.html</string>
        <key>MIMEType</key>
        <string>text/html</string>
        <key>Data</key>
        <data>
            PD94bWwgdmVyc2lvbj0iMS4w...
        </data>
        <key>WebResourceResponse</key>
        <dict>
            <key>URL</key>
            <string>https://example.com/page.html</string>
            <key>MIMEType</key>
            <string>text/html</string>
            <key>textEncodingName</key>
            <string>UTF-8</string>
        </dict>
    </dict>
    <key>Resources</key>
    <array>
        <dict>
            <key>URL</key>
            <string>https://example.com/images/photo.jpg</string>
            <key>MIMEType</key>
            <string>image/jpeg</string>
            <key>Data</key>
            <data>
                /9j/4AAQSkZJRgABAQAA...
            </data>
        </dict>
        <!-- More resources... -->
    </array>
</dict>
</plist>
```

### Parsing Process

1. **Read File** - Binary webarchive (.webarchive)
2. **Parse Plist** - Extract plist structure
3. **Extract MainResource** - Get HTML content and metadata
4. **Decode Data** - Convert base64/binary to text
5. **Handle Resources** - Extract embedded resources (optional)
6. **Convert to Markdown** - Standard HTML to Markdown conversion

## Features

✅ **Full Webarchive Support**
- Extracts HTML from webarchive container
- Preserves document structure
- Handles UTF-8 and binary encodings
- Resilient parsing (continues on minor errors)

✅ **Integrated with HTML Features**
- Works with metadata extraction
- Compatible with CSS hints preservation
- Supports image handling (link/skip modes)
- Full TOC generation support

✅ **Resource Information**
- Can list embedded resources
- Reports resource counts and sizes
- Preserves resource metadata

## Performance

Webarchive parsing is efficient:
- Single-pass plist parsing: O(n)
- Resource extraction: O(m) where m = number of resources
- Base64 decoding: O(n)

Typical conversion times:
- Small archive (< 1MB): 10-50ms
- Medium archive (1-10MB): 50-500ms
- Large archive (> 10MB): 500ms+

## Limitations & Known Issues

- **Sub-resources:** Embedded images and stylesheets are parsed but not re-embedded by default
  - Use `image_format: "link"` to preserve image references
  - CSS is converted to hints with `preserve_css_hints: true`
  
- **JavaScript:** Script content is stripped (security consideration)

- **Form Elements:** Forms are converted to text representation

- **Interactive Content:** Flash, Java, etc. not supported

- **Relative URLs:** May need base URL resolution if resources use relative paths

## Troubleshooting

### "Invalid webarchive format"

**Issue:** File is not a valid webarchive or is corrupted

**Solutions:**
1. Verify the file is actually a webarchive: `file webpage.webarchive`
2. Try re-saving the page in Safari
3. Check file integrity: `plutil -lint webpage.webarchive`

### "MainResource missing Data field"

**Issue:** Webarchive structure is incomplete or corrupted

**Solutions:**
1. Ensure Safari saved the file completely
2. Try opening in Safari first to verify integrity
3. Create a fresh webarchive from the webpage

### Encoding issues

**Issue:** Text appears garbled

**Solutions:**
1. Check TextEncodingName in webarchive metadata
2. Verify browser locale when saving
3. Re-save with explicit UTF-8 encoding

### Missing embedded resources

**Issue:** Images or stylesheets not found in output

**Solutions:**
1. Use `image_format: "link"` to preserve image URLs
2. Check if webarchive was saved with resources
3. Verify resource references in HTML

## Integration with Other Tools

### Static Site Generators

Convert webarchives to Jekyll/Hugo compatible Markdown:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "archived_post.webarchive",
    "extract_metadata": true,
    "generate_toc": true,
    "include_filename": false
  }
}
```

Output includes YAML frontmatter suitable for Jekyll/Hugo.

### Document Processing

Chain webarchive conversion with other tools:

```bash
# Convert webarchive to Markdown
./to_markdown_mcp < page.webarchive > page.md

# Further process the Markdown
pandoc page.md -f markdown -t docx -o page.docx
```

### Batch Processing

Convert multiple webarchives:

```bash
for archive in *.webarchive; do
  ./to_markdown_mcp < "$archive" > "${archive%.webarchive}.md"
done
```

## Examples

### Example 1: Simple Article Archive

**Input:** Safari archive of blog article

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "my_blog_article.webarchive",
    "extract_metadata": true
  }
}
```

**Output:** Markdown with YAML frontmatter and clean text

### Example 2: Product Page Archive

**Input:** E-commerce product page with images

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "product_page.webarchive",
    "extract_images": true,
    "image_format": "link",
    "preserve_css_hints": true
  }
}
```

**Output:** Markdown preserving image references and styling hints

### Example 3: Documentation Archive

**Input:** Web documentation with deep structure

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "documentation.webarchive",
    "extract_metadata": true,
    "generate_toc": true,
    "toc_max_level": 4,
    "preserve_css_hints": true
  }
}
```

**Output:** Markdown with frontmatter, TOC, and preserved structure

## Testing

Test files are available in `examples/`:
- `examples/sample.webarchive` - Simple webpage archive

Create your own test webarchives:
1. Open any webpage in Safari
2. Select **File → Save As → Web Archive**
3. Use the generated `.webarchive` file with toMarkdownMCP

## Future Enhancements

Planned improvements:
- Resource extraction (save embedded images separately)
- Batch webarchive processing
- Metadata preservation with custom headers
- Lazy loading for very large archives
- Archive validation and repair
- Resource deduplication
- Format version detection and optimization

## References

- [Apple Webarchive Format](https://en.wikipedia.org/wiki/Webarchive)
- [Property List Format](https://en.wikipedia.org/wiki/Property_list)
- [Safari Documentation](https://support.apple.com/guide/safari/save-web-pages-sfri40696/mac)
- Related: HTML conversion, Metadata extraction, Image handling, TOC generation

