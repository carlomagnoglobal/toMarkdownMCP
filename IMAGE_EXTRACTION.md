# Image Extraction and Processing

Automatically extract and process images from HTML documents with configurable output formats.

## Overview

The image extraction feature allows you to control how images in HTML documents are handled during conversion to Markdown:

- **Link format** - Keep external image URLs, generate Markdown image syntax
- **Skip format** - Remove images, replace with alt text placeholders
- **Embed format** - Convert images to base64 for inline embedding (planned)

This is useful for:

- **Offline documentation** - Embed images for self-contained documents
- **Clean exports** - Skip images for text-only exports
- **Web content** - Preserve image links for web-based content
- **Accessibility** - Maintain alt text descriptions

## How It Works

1. **Parse HTML** - Scan for `<img>` tags
2. **Extract metadata** - Collect src, alt, and title attributes
3. **Process images** - Apply format transformation
4. **Generate output** - Create Markdown image syntax

## Usage

### With convert_file Tool

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "document.html",
    "extract_images": true,
    "image_format": "link"
  }
}
```

### With convert_from_source Tool

```json
{
  "name": "convert_from_source",
  "arguments": {
    "source": "https://example.com/article.html",
    "extract_images": true,
    "image_format": "skip"
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
    "extract_images": true,
    "image_format": "link",
    "preserve_css_hints": true,
    "generate_toc": true
  }
}
```

## Parameters

### `extract_images` (boolean)
- **Default:** `false`
- **Description:** Enable/disable image extraction and processing
- **Values:** `true` or `false`

### `image_format` (string)
- **Default:** `"link"`
- **Description:** How to handle images in output
- **Values:**
  - `"link"` - Preserve as Markdown image links
  - `"skip"` - Remove images, keep alt text
  - `"embed"` - Convert to base64 (future)

## Output Format

### Link Format (Default)

**Input:**
```html
<img src="https://example.com/photo.jpg" alt="Example Photo" title="Photo Title">
```

**Output:**
```markdown
![Example Photo](https://example.com/photo.jpg)  
*Photo Title*
```

### Skip Format

**Input:**
```html
<img src="image.jpg" alt="Important graphic">
```

**Output:**
```markdown
*[Image: Important graphic]*
```

### Embed Format (Planned)

**Input:**
```html
<img src="logo.png" alt="Logo">
```

**Output:**
```markdown
![Logo](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgA...)
```

## Image Attribute Handling

### Source Attribute (`src`)

- **Relative URLs** - Preserved as-is (works with local HTML)
- **Absolute URLs** - Preserved for linking
- **Data URLs** - Preserved if already embedded
- **Missing src** - Image skipped

### Alt Text (`alt`)

- **Present** - Used as Markdown alt text
- **Missing** - Defaults to "Image"
- **Empty** - Still used (provides empty alt)

### Title Attribute (`title`)

- **Present** - Added as caption below image (link format only)
- **Missing** - Omitted from output
- **Format** - Italicized caption text

## Integration with Other Features

### With Metadata Extraction

When both enabled, order is preserved:

```json
{
  "extract_metadata": true,
  "extract_images": true,
  "image_format": "link"
}
```

Output:
1. YAML metadata
2. Document content with images as links

### With CSS Styling Hints

Image processing and CSS hints work independently:
- Images processed first
- CSS hints preserved in remaining HTML
- No interference between features

### With Table of Contents

TOC reflects document structure after image processing:
- Images don't affect heading structure
- TOC generated from headings regardless of images
- Optimal order: metadata → TOC → images → content

## Examples

### Example 1: Photo Article

**HTML:**
```html
<article>
  <h1>Travel Guide</h1>
  <img src="/photos/paris.jpg" alt="Eiffel Tower" title="Iconic landmark">
  <p>Paris is beautiful...</p>
  <img src="/photos/louvre.jpg" alt="Louvre Museum">
  <p>The museum houses...</p>
</article>
```

**With extract_images=true, image_format="link":**
```markdown
# Travel Guide

![Eiffel Tower](/photos/paris.jpg)  
*Iconic landmark*

Paris is beautiful...

![Louvre Museum](/photos/louvre.jpg)

The museum houses...
```

### Example 2: Technical Documentation

**HTML:**
```html
<h1>API Reference</h1>
<p>Here's the architecture:</p>
<img src="diagram.png" alt="System Architecture">
<p>See how components interact...</p>
```

**With extract_images=true, image_format="skip":**
```markdown
# API Reference

Here's the architecture:

*[Image: System Architecture]*

See how components interact...
```

### Example 3: With Metadata

**HTML:**
```html
<!DOCTYPE html>
<head>
  <meta name="author" content="Jane Doe">
  <title>Photo Essay</title>
</head>
<body>
  <h1>My Journey</h1>
  <img src="photo1.jpg" alt="Photo 1">
  <p>The journey began...</p>
</body>
</html>
```

**With extract_metadata=true, extract_images=true:**
```markdown
---
author: Jane Doe
title: Photo Essay
---

# My Journey

![Photo 1](photo1.jpg)

The journey began...
```

## Image Format Details

### Link Format

**Advantages:**
- ✅ Preserves all image information
- ✅ Works with URLs and local paths
- ✅ No encoding overhead
- ✅ Small output size
- ✅ Easy to batch-process images

**Disadvantages:**
- ❌ Images not self-contained
- ❌ Broken links if URLs change
- ❌ Requires network access to view

**Best for:** Web content, online documentation, image galleries

### Skip Format

**Advantages:**
- ✅ Minimal file size
- ✅ Text-only content
- ✅ Fast processing
- ✅ Useful for analysis/parsing
- ✅ Preserves alt text

**Disadvantages:**
- ❌ Loses visual content
- ❌ No image information
- ❌ Less useful for visual media

**Best for:** Text extraction, archiving, accessibility-first docs

### Embed Format (Planned)

**Advantages:**
- ✅ Self-contained documents
- ✅ Works offline
- ✅ No external dependencies
- ✅ Great for email/messaging

**Disadvantages:**
- ❌ Large file size (base64 adds ~33%)
- ❌ Slower encoding
- ❌ Limited browser support for very large images
- ❌ Requires async processing

**Best for:** Self-contained exports, email attachments, offline docs

## Implementation Details

### Functions

**extract_image_urls()**
- Parses HTML for img tags
- Returns Vec of ExtractedImage structs
- Preserves all attributes

**generate_image_markdown()**
- Converts ExtractedImage to Markdown
- Handles format-specific output
- Includes captions for titled images

**process_images_in_html()**
- Orchestrator function
- Applies format transformation
- Returns processed HTML

**image_to_data_url()**
- Converts image to base64
- Detects MIME type
- Handles URL and local files (async)

### URL Handling

**Relative URLs:**
```html
<img src="images/photo.jpg">
<!-- Becomes -->
![alt](images/photo.jpg)
```

**Absolute URLs:**
```html
<img src="https://cdn.example.com/img.png">
<!-- Becomes -->
![alt](https://cdn.example.com/img.png)
```

**Data URLs:**
```html
<img src="data:image/png;base64,iVBORw0...">
<!-- Preserved as-is -->
```

## Testing

Included example: `examples/images_demo.html`

Test different formats:

```bash
# Preserve image links (recommended for web content)
extract_images: true
image_format: "link"

# Remove images entirely
extract_images: true
image_format: "skip"

# Disable image extraction (default)
extract_images: false
```

## Performance

Image processing is efficient:
- Single document scan
- O(n) HTML parsing (n = document size)
- No network calls for link format
- Minimal memory overhead

Typical times:
- 10KB document: < 1ms
- 100KB document: 1-2ms
- 1MB document: 5-10ms

(Embed format would add base64 encoding time: +2-5ms per image)

## Limitations

- Only processes standard `<img>` tags
- No support for `<picture>`, `<svg>`, `<canvas>`
- Alt text is required for meaningful output
- Relative URLs depend on context
- Base64 embed not yet implemented
- No image optimization/resizing
- No watermark/metadata removal

## Future Enhancements

Planned improvements:
- Base64 embed implementation (async)
- Image size thresholds for auto-embedding
- Format detection for untyped URLs
- Relative URL resolution (with base URL)
- Image alt text generation
- Picture tag support
- Inline SVG conversion
- AVIF and modern format support
- Image deduplication
- CDN/cache integration

## Troubleshooting

### Images Not Processed

**Issue:** `extract_images` is `false` by default
**Solution:** Set `extract_images: true` in parameters

### Missing Alt Text

**Issue:** Markdown image without alt becomes `![](...)`
**Solution:** Ensure source HTML has meaningful alt attributes

### Broken Image Links

**Issue:** Relative URLs break in different contexts
**Solution:** Use `image_format: "skip"` or adjust relative path base

### Large File Sizes

**Issue:** Many external image links create large Markdown
**Solution:** Consider `image_format: "skip"` for text extraction

### Relative URLs Not Working

**Issue:** `<img src="image.jpg">` breaks in different locations
**Solution:** Update to absolute URLs or use base URL resolution (future)

## References

- Markdown image syntax
- HTML img tag specification
- Base64 encoding
- MIME type detection
- Related: Metadata extraction, CSS hints, TOC generation

