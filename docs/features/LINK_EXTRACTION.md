# Link Extraction

Extract and analyze all hyperlinks from HTML documents, categorizing them as external, internal, or broken.

## Overview

The link extraction feature identifies all links in HTML documents and generates a comprehensive summary with link statistics and categorization. This is useful for:

- **Link Auditing** - Find all links in a document
- **Link Validation** - Identify broken or invalid links
- **Content Analysis** - Understand document link structure
- **Reference Documentation** - Extract all URLs for documentation
- **SEO Analysis** - Analyze link distribution and targets
- **Content Migration** - Preserve links during conversion

## How It Works

1. **Parse HTML** - Find all `<a>` tag elements
2. **Extract Metadata** - Get URL, text, title, target, rel attributes
3. **Classify Links** - Determine if external, internal, or broken
4. **Generate Summary** - Create readable summary by category
5. **Include References** - Generate Markdown reference format

## Link Types

### External Links
Links that go outside the current domain:
- Full URLs with http/https protocol
- Protocol-relative URLs (starting with //)
- Links to different domains

### Internal Links
Links within the same site/document:
- Relative paths (/page, ../file)
- URLs without protocol on same domain

### Broken/Invalid Links
Links that don't point to valid content:
- Empty href attributes
- Pure anchor links (#section)
- JavaScript protocol (javascript:)
- No href at all

## Usage

### Extract and Summarize Links

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "extract_links": true
  }
}
```

### With Other Features

Combine link extraction with other features:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "extract_links": true,
    "extract_metadata": true,
    "preserve_comments": true,
    "convert_tables": true
  }
}
```

## Output Format

### Input HTML
```html
<html>
  <head>
    <title>My Page</title>
  </head>
  <body>
    <h1>Welcome</h1>
    <a href="https://example.com">External Link</a>
    <a href="/about">Internal Link</a>
    <a href="">Broken Link</a>
    <a href="#section">Anchor Link</a>
  </body>
</html>
```

### Output Markdown with Links Summary

```markdown
# My Page

## Links Found

**Total:** 3 links

**External:** 1 | **Internal:** 1 | **Broken:** 1

### External Links

1. [External Link](https://example.com)

### Internal Links

1. [Internal Link](/about)

### Broken/Invalid Links

1. `Broken Link`

# Welcome

Welcome text here.
```

## Link Information Captured

Each link extraction includes:

- **URL** - The href attribute value
- **Text** - The visible link text
- **Title** - Optional title attribute
- **Target** - Link target (_blank, _self, etc.)
- **Rel** - Relationship (nofollow, noopener, etc.)
- **Classification** - External, internal, or broken
- **External Flag** - Whether link goes to external domain
- **Broken Flag** - Whether link appears invalid

## Examples

### Blog Post with Links

**HTML:**
```html
<article>
  <h1>Getting Started with Markdown</h1>
  
  <p>Read more about <a href="https://guides.github.com/features/mastering-markdown/">Markdown</a> on GitHub.</p>
  
  <h2>Resources</h2>
  <ul>
    <li><a href="https://www.markdownguide.org/">Markdown Guide</a></li>
    <li><a href="https://daringfireball.net/projects/markdown/">Original Markdown</a></li>
    <li><a href="/markdown-syntax">Our Syntax Guide</a></li>
  </ul>
  
  <p><a href="#top">Back to top</a></p>
</article>
```

**Link Summary:**
```markdown
## Links Found

**Total:** 4 links

**External:** 3 | **Internal:** 1

### External Links

1. [Markdown](https://guides.github.com/features/mastering-markdown/)
2. [Markdown Guide](https://www.markdownguide.org/)
3. [Original Markdown](https://daringfireball.net/projects/markdown/)

### Internal Links

1. [Our Syntax Guide](/markdown-syntax)
```

### E-commerce Product Page

**HTML:**
```html
<div class="product">
  <h1>Laptop Computer</h1>
  
  <a href="">Out of Stock</a>
  
  <p>
    Related Products:
    <a href="/products/mouse" title="Wireless Mouse">Mouse</a>,
    <a href="/products/keyboard" title="Mechanical Keyboard">Keyboard</a>
  </p>
  
  <p>
    <a href="https://support.example.com/" target="_blank" rel="noopener">Support</a>
  </p>
</div>
```

**Link Summary:**
```markdown
## Links Found

**Total:** 3 links

**External:** 1 | **Internal:** 2 | **Broken:** 1

### External Links

1. [Support](https://support.example.com/) - "Support"

### Internal Links

1. [Mouse](/products/mouse) - "Wireless Mouse"
2. [Keyboard](/products/keyboard) - "Mechanical Keyboard"

### Broken/Invalid Links

1. `Out of Stock`
```

## Processing Order

When combined with other features:
1. **Link Extraction** (first - analyzes links)
2. **Comment Preservation** (extracts comments)
3. **Form Extraction** (processes forms)
4. **Table Conversion** (converts tables)
5. **Image Extraction** (processes images)
6. **HTML to Markdown** (final conversion)

## Link Statistics

The extractor provides statistics including:
- Total number of links
- External vs internal count
- Links with titles
- Links with targets
- Broken/invalid links

## Implementation Details

### Module Structure

```rust
pub struct ExtractedLink {
    pub url: String,
    pub text: String,
    pub title: Option<String>,
    pub rel: Option<String>,
    pub target: Option<String>,
    pub is_external: bool,
    pub is_broken: Option<bool>,
}

pub fn extract_links_from_html(html: &str) -> Result<Vec<ExtractedLink>>
pub fn generate_link_summary(links: &[ExtractedLink]) -> String
pub fn get_link_statistics(links: &[ExtractedLink]) -> LinkStatistics
```

### Link Classification

- **External:** Protocol starts with http://, https://, or //
- **Internal:** Relative path without external protocol
- **Broken:** Empty href, starts with #, or javascript:

### Text Extraction

- Gets visible link text from HTML content
- Handles formatted text (bold, italic, etc.)
- Escapes special characters (pipes, quotes)
- Truncates long text in summaries

## Limitations

- **JavaScript Links** - Links created by JavaScript are not captured
- **Dynamic Content** - Links added after page load are not included
- **Relative URLs** - May be ambiguous without knowing base URL
- **Redirect Chains** - Direct links, not final destinations
- **Fragment Identifiers** - Anchor-only links are not included

## Capabilities vs Limitations

### What Works Well
- Standard http/https links ✅
- Relative paths (/page, ../file) ✅
- Protocol-relative URLs (//) ✅
- Links with attributes (title, target, rel) ✅
- Multiple links per page ✅
- Mixed internal/external links ✅

### What Has Limitations
- JavaScript-injected links
- Links in comments
- Links in CSS/scripts
- Dynamically generated URLs
- Links requiring authentication

## Testing

Link extraction can be tested with any HTML file containing links:

```bash
# Test with any HTML file
cargo build --release
echo '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"test.html","extract_links":true}}}' | ./target/release/to_markdown_mcp
```

Example test HTML:
```html
<html>
<body>
  <a href="https://example.com">External</a>
  <a href="/page">Internal</a>
  <a href="">Broken</a>
  <a href="#section">Anchor</a>
</body>
</html>
```

## Integration with Other Features

### With Metadata Extraction
Links extracted independently of page metadata.

### With Comment Preservation
Comments in/around links are preserved in summary.

### With Form Extraction
Links in forms are captured separately.

### With Image Extraction
Links to images are included in link summary.

### With Table Conversion
Links in tables are preserved.

## Future Enhancements

Planned improvements:
- Link validation (check if URLs are reachable)
- Anchor link resolution (find target sections)
- Relative URL resolution (convert to absolute)
- Link categorization (external types)
- Protocol distribution analysis
- Domain frequency analysis
- Dead link detection
- Link relationship analysis (nofollow, sponsored, etc.)
- Export to OPML or other formats
- Integration with link checkers

## References

- [HTML Links Specification](https://html.spec.whatwg.org/#links)
- [Link Relationships](https://html.spec.whatwg.org/#linkTypes)
- [URL Specification](https://url.spec.whatwg.org/)
- Related: Comment preservation, Form extraction, Table conversion
