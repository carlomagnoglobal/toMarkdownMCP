# Comment Preservation

Preserve and extract HTML comments during conversion to Markdown, capturing developer notes, metadata, and conditional logic.

## Overview

The comment preservation feature identifies and preserves HTML comments from documents, converting them to readable Markdown format. This is useful for:

- **Developer Notes** - Preserve inline documentation and notes
- **Audit Trail** - Keep track of comments about content or changes
- **Conditional Logic** - Document browser-specific code (IE conditional comments)
- **Metadata** - Extract structured comments for analysis
- **Content Migration** - Preserve all document information including comments

## How It Works

1. **Scan HTML** - Find all `<!-- -->` comment blocks
2. **Classify** - Detect comment type (regular, directive, conditional)
3. **Extract** - Preserve comment content and metadata
4. **Generate Summary** - Create readable summary of all comments
5. **Convert** - Represent comments in Markdown output

## Comment Types

### Regular Comments
Standard HTML comments used for notes and documentation:
```html
<!-- This is a regular comment -->
<!-- TODO: Update this section -->
<!-- Authors: John, Jane -->
```

### Directive Comments
Comments with special meaning or structure:
```html
<!-- DOCTYPE html -->
<!-- Meta: author=John -->
<!-- Configuration: enabled=true -->
```

### Conditional Comments (IE)
Internet Explorer conditional comments for browser-specific code:
```html
<!--[if IE]>
    <p>Internet Explorer only</p>
<![endif]-->

<!--[if lt IE 9]>
    <script src="html5shiv.js"></script>
<![endif]-->
```

## Usage

### Preserve Comments in Output

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "preserve_comments": true
  }
}
```

### With Other Features

Combine comment preservation with other features:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "preserve_comments": true,
    "extract_metadata": true,
    "convert_tables": true,
    "generate_toc": true
  }
}
```

## Output Format

### Input HTML
```html
<!DOCTYPE html>
<html>
<!-- Main page container -->
<head>
    <!-- Metadata -->
    <title>Page Title</title>
</head>
<body>
    <!--[if IE]><p>IE only message</p><![endif]-->
    <!-- Content section -->
    <h1>Welcome</h1>
    <!-- TODO: Add more content here -->
    <p>Page content here.</p>
</body>
</html>
```

### Output Markdown with Comments

```markdown
# page.html

## Comments Found

**Total:** 5 comments

**Conditional:** 1 | **Regular:** 4

### Directives

1. `DOCTYPE html`

### Conditional Comments (IE)

1. `[if IE]><p>IE only message</p><![endif]`

### Regular Comments

1. `Main page container`
2. `Metadata`
3. `Content section`
4. `TODO: Add more content here`

# Welcome

Page content here.
```

## Processing Order

When combined with other features:
1. **Comment Preservation** (first - extracts and summarizes)
2. **Form Extraction** (processes forms)
3. **Table Conversion** (converts tables)
4. **Image Extraction** (processes images)
5. **HTML to Markdown** (final conversion)

## Comment Summary Format

The comment summary includes:

- **Total Count** - Number of comments found
- **By Type** - Breakdown (directives, conditional, regular)
- **Directives Section** - Special comments
- **Conditional Section** - IE conditional comments
- **Regular Section** - Standard comments

Each comment is listed with content preview (first 80 characters).

## Examples

### Documentation Comments

**HTML:**
```html
<!-- 
    Page: User Dashboard
    Author: John Smith
    Version: 2.0
    Last Updated: 2024-01-15
-->
<html>
  <!-- Header section with logo and navigation -->
  <header>
    <h1>User Dashboard</h1>
  </header>
</html>
```

**Comment Summary:**
```markdown
## Comments Found

**Total:** 2 comments

### Regular Comments

1. `Page: User Dashboard Author: John Smith Version: 2.0 La...`
2. `Header section with logo and navigation`
```

### Conditional Browser Code

**HTML:**
```html
<!--[if IE 6]>
    <link rel="stylesheet" href="ie6.css">
<![endif]-->

<!--[if lt IE 9]>
    <script src="html5shiv.js"></script>
<![endif]-->

<!-- Standard CSS for modern browsers -->
<link rel="stylesheet" href="style.css">
```

**Comment Summary:**
```markdown
## Comments Found

**Total:** 3 comments

**Conditional:** 2 | **Regular:** 1

### Conditional Comments (IE)

1. `[if IE 6]> <link rel="stylesheet" href="ie6.css"> <...`
2. `[if lt IE 9]> <script src="html5shiv.js"></script> <...`

### Regular Comments

1. `Standard CSS for modern browsers`
```

### TODO Comments

**HTML:**
```html
<html>
  <head>
    <!-- TODO: Update page description -->
    <meta name="description" content="Page description">
  </head>
  <body>
    <!-- TODO: Add footer section -->
    <!-- TODO: Update copyright year to 2025 -->
    <footer>
      <p>Copyright 2024</p>
    </footer>
  </body>
</html>
```

**Comment Summary:**
```markdown
## Comments Found

**Total:** 3 comments

### Regular Comments

1. `TODO: Update page description`
2. `TODO: Add footer section`
3. `TODO: Update copyright year to 2025`
```

## Implementation Details

### Module Structure

```rust
pub struct HtmlComment {
    pub content: String,
    pub is_directive: bool,
    pub is_conditional: bool,
}

pub fn extract_comments_from_html(html: &str) -> Result<Vec<HtmlComment>>
pub fn generate_comment_summary(comments: &[HtmlComment]) -> String
pub fn process_comments_in_html(html: &str, preserve: bool) -> Result<(String, Vec<HtmlComment>)>
```

### Comment Detection

- **Regular Comments** - Standard `<!-- ... -->`
- **Directives** - Detected by `[` prefix or `DOCTYPE` keyword
- **Conditional** - Detected by `[if ...` and `[endif]` patterns

### Preserving Comments

When `preserve_comments=true`:
1. Comments remain in HTML as-is
2. Summary is generated and prepended to output
3. Comments appear in both summary and converted HTML

## Limitations

- **Comment Structure** - Comments with `-->` inside content may break (rare)
- **Dynamic Comments** - Comments generated by JavaScript are not captured
- **Performance** - Large documents with many comments process slower
- **Encoding** - Non-UTF8 comment content may not display correctly

## Capabilities vs Limitations

### What Works Well
- Regular HTML comments ✅
- IE conditional comments ✅
- Comments with multiple lines ✅
- Comments with special characters ✅
- Mixed comment types ✅

### What Has Limitations
- Nested `-->` sequences in comment content
- CDATA sections with comment-like syntax
- Comments in attribute values
- Dynamically injected comments

## Testing

Comment extraction can be tested with any HTML file containing comments:

```bash
# Test with any HTML file
cargo build --release
echo '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"test.html","preserve_comments":true}}}' | ./target/release/to_markdown_mcp
```

Example test HTML:
```html
<!DOCTYPE html>
<html>
<!-- Page comment -->
<head>
    <!-- Meta comment -->
    <title>Test</title>
</head>
<body>
    <!--[if IE]><p>IE Only</p><![endif]-->
    <h1>Content</h1>
</body>
</html>
```

## Integration with Other Features

### With Metadata Extraction
Comments extracted independently of metadata.

### With Image Extraction
Comments preserved regardless of image processing.

### With Form Extraction
Comments in forms are preserved in summary.

### With Table Conversion
Comments in/around tables are preserved.

### With CSS Hints
CSS comments are regular comments (no special handling).

## Future Enhancements

Planned improvements:
- Comment type categorization (TODO, BUG, NOTE, etc.)
- Extracting structured metadata from comments
- Filtering comments by type
- Generating index of all TODOs and FIXMEs
- Comment-based field descriptions
- Integration with issue tracking systems
- Support for comment-based annotations
- Markdown comment format options

## References

- [HTML Comments Specification](https://html.spec.whatwg.org/#comments)
- [IE Conditional Comments](https://docs.microsoft.com/en-us/previous-versions/windows/internet-explorer/ie-developer/compatibility/ms537512)
- [Markdown Comments](https://www.markdownguide.org/faq/#can-i-use-html-comments-in-markdown)
- Related: Form extraction, Table conversion, Metadata extraction
