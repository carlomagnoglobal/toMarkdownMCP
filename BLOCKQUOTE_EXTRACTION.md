# Blockquote Extraction

Extract and convert HTML blockquotes to Markdown format.

## Overview

The blockquote extractor identifies `<blockquote>` elements and converts them to Markdown blockquote syntax. Useful for:

- **Quotes** - Extracting attributed quotations
- **Content** - Preserving quoted material during conversion
- **Citations** - Capturing source information

## Usage

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "extract_blockquotes": true
  }
}
```

## Output

**Input HTML:**
```html
<blockquote>
  The only way to do great work is to love what you do.
  <cite>Steve Jobs</cite>
</blockquote>
```

**Output Markdown:**
```markdown
## Blockquotes Found

**Total:** 1 blockquotes

**With Citation:** 1

### Preview (First 3)

1. "The only way to do great work is to love what you..." — Steve Jobs

> The only way to do great work is to love what you do.
> — Steve Jobs
```

## Supported Attributes

- `cite` - URL reference
- `data-author` - Author attribution
- `<cite>` - Citation element
- `<footer>` - Footer/attribution

## Processing Order

Blockquotes are processed first in the pipeline, before other content extraction.
