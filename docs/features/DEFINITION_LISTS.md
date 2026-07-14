# Definition Lists

Convert HTML definition lists (`<dl>`, `<dt>`, `<dd>`) to Markdown format.

## Overview

The definition list converter extracts and transforms HTML definition lists into readable Markdown. Definition lists are useful for:

- **Glossaries** - Definitions and terms
- **FAQ** - Questions and answers
- **Specifications** - Terms and descriptions
- **Content Migration** - Converting HTML reference material

## HTML Definition Lists

### Structure

```html
<dl>
  <dt>Term</dt>
  <dd>Definition</dd>
  
  <dt>Another Term</dt>
  <dd>Its definition</dd>
</dl>
```

### Multiple Definitions

```html
<dl>
  <dt>HTML</dt>
  <dd>HyperText Markup Language</dd>
  <dd>Used for creating web pages</dd>
</dl>
```

### Multiple Terms

```html
<dl>
  <dt>HTTP</dt>
  <dt>HTTPS</dt>
  <dd>Protocols for transferring web data</dd>
</dl>
```

## Usage

### Extract Definition Lists

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "glossary.html",
    "extract_definition_lists": true
  }
}
```

### With Other Features

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "extract_definition_lists": true,
    "analyze_headings": true,
    "extract_links": true
  }
}
```

## Output Format

### Input HTML
```html
<dl>
  <dt>HTML</dt>
  <dd>HyperText Markup Language for creating web pages</dd>
  
  <dt>CSS</dt>
  <dd>Cascading Style Sheets for styling</dd>
</dl>
```

### Output Markdown (Expanded)
```markdown
## Definition Lists Found

**Total:** 1 definition lists

**Definitions:** 2
**Descriptions:** 2

### Preview (First List)

1. **HTML** - HyperText Markup Language for creating...
2. **CSS** - Cascading Style Sheets for styling

**HTML**
:   HyperText Markup Language for creating web pages

**CSS**
:   Cascading Style Sheets for styling
```

### Compact Format

For short definitions (single line, < 100 chars):

```markdown
**HTML** : HyperText Markup Language
**CSS** : Cascading Style Sheets
```

## Conversion Modes

The converter supports two output formats:

### 1. Expanded Format (Default)
Uses Markdown definition list syntax:
```
**Term**
:   Description
```

Best for longer definitions and readability.

### 2. Compact Format
Uses inline format:
```
**Term** : Description
```

Used automatically for lists with short descriptions.

### 3. Table Format (Alternative)
```
| Term | Definition |
|------|------------|
| HTML | HyperText Markup Language |
```

## Features

- ✅ Extracts all definition lists
- ✅ Handles multiple definitions per term
- ✅ Handles multiple terms per definition
- ✅ Escapes special characters (pipes)
- ✅ Detects compact vs. expanded format
- ✅ Generates summary statistics
- ✅ Preserves formatting in descriptions

## Examples

### Simple Glossary

**HTML:**
```html
<dl>
  <dt>SEO</dt>
  <dd>Search Engine Optimization</dd>
  
  <dt>API</dt>
  <dd>Application Programming Interface</dd>
  
  <dt>REST</dt>
  <dd>Representational State Transfer</dd>
</dl>
```

**Markdown Output:**
```markdown
**SEO**
:   Search Engine Optimization

**API**
:   Application Programming Interface

**REST**
:   Representational State Transfer
```

### FAQ Format

**HTML:**
```html
<dl>
  <dt>What is HTML?</dt>
  <dd>HTML is the standard markup language for creating web pages.</dd>
  
  <dt>Is HTML a programming language?</dt>
  <dd>No, HTML is a markup language, not a programming language.</dd>
</dl>
```

**Markdown:**
```markdown
**What is HTML?**
:   HTML is the standard markup language for creating web pages.

**Is HTML a programming language?**
:   No, HTML is a markup language, not a programming language.
```

## Processing Order

When combined with other features:
1. **Definition List Extraction** (first)
2. **Heading Analysis**
3. **Link Extraction**
4. **Comment Preservation**
5. **Form Extraction**
6. **Table Conversion**
7. **Image Extraction**
8. **HTML to Markdown**

## Statistics

The converter generates a summary including:
- Total number of definition lists
- Total definitions extracted
- Total descriptions
- Preview of first few terms

## Implementation Details

### Module Structure

```rust
pub struct Definition {
    pub term: String,
    pub descriptions: Vec<String>,
}

pub struct DefinitionList {
    pub definitions: Vec<Definition>,
    pub compact: bool,
}

pub fn extract_definition_lists_from_html(html: &str) -> Result<Vec<DefinitionList>>
pub fn definition_list_to_markdown(list: &DefinitionList) -> String
pub fn definition_list_to_table(list: &DefinitionList) -> String
```

### Parsing Algorithm

1. Process all `<dt>` and `<dd>` elements in order
2. Group consecutive `<dd>` elements after `<dt>` elements
3. Multiple terms can share descriptions
4. Multiple descriptions can apply to single term

## Limitations

- **No styling** - Description styling not preserved (bold, italic, etc.)
- **Simple grouping** - Assumes standard dl/dt/dd structure
- **No nesting** - Doesn't support nested definition lists
- **JavaScript** - Lists created by JavaScript not captured

## Capabilities

- ✅ Standard HTML definition lists
- ✅ Multiple terms sharing definitions
- ✅ Multiple definitions per term
- ✅ Formatted text in descriptions
- ✅ Links and nested HTML in descriptions
- ✅ Long and short definitions

## Testing

Definition list extraction works with any HTML containing `<dl>` elements:

```bash
echo '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"glossary.html","extract_definition_lists":true}}}' | ./target/release/to_markdown_mcp
```

## Future Enhancements

Planned improvements:
- Support for nested definition lists
- Custom markdown output formats
- Term frequency analysis
- Definition length statistics
- Automatic glossary generation
- Table of contents for definitions

## References

- [HTML Definition Lists Spec](https://html.spec.whatwg.org/#the-dl-element)
- [Markdown Definition Lists](https://pandoc.org/MANUAL.html#definition-lists)
- Related: Heading analysis, Link extraction, Form extraction
