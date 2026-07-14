# HTML Table Conversion to Markdown

Automatically convert HTML tables to clean Markdown pipe table format.

## Overview

The table conversion feature extracts HTML tables and converts them to Markdown pipe tables, preserving content and structure. This is useful for:

- **Documentation** - Convert web-based documentation to Markdown
- **Data Export** - Extract HTML tables from websites
- **Static Site Generation** - Create Markdown-based table content
- **Content Migration** - Move HTML content to Markdown platforms
- **Comparison Tables** - Convert product/feature comparison tables
- **Reporting** - Convert HTML reports to Markdown format

## How It Works

1. **Parse HTML** - Find all `<table>` elements
2. **Extract Cells** - Get headers and data from `<th>` and `<td>` elements
3. **Format Text** - Extract and clean text from cell content
4. **Handle Formatting** - Process formatted content (bold, italic, etc.)
5. **Generate Markdown** - Create pipe table syntax with proper alignment

## Usage

### With convert_file Tool

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page_with_tables.html",
    "convert_tables": true
  }
}
```

### With convert_from_source Tool

```json
{
  "name": "convert_from_source",
  "arguments": {
    "source": "https://example.com/data.html",
    "convert_tables": true
  }
}
```

### Combined with Other Features

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "article.html",
    "convert_tables": true,
    "extract_metadata": true,
    "generate_toc": true,
    "extract_images": true
  }
}
```

## Parameters

### `convert_tables` (boolean)
- **Default:** `false`
- **Description:** Enable/disable HTML to Markdown table conversion
- **Values:** `true` or `false`

## Output Format

### Simple Table

**Input HTML:**
```html
<table>
  <tr>
    <th>Name</th>
    <th>Age</th>
  </tr>
  <tr>
    <td>Alice</td>
    <td>30</td>
  </tr>
  <tr>
    <td>Bob</td>
    <td>25</td>
  </tr>
</table>
```

**Output Markdown:**
```markdown
| Name | Age |
| :--- | :--- |
| Alice | 30 |
| Bob | 25 |
```

### Structured Table (with thead/tbody)

**Input HTML:**
```html
<table>
  <thead>
    <tr>
      <th>Product</th>
      <th>Price</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>Apple</td>
      <td>$1.00</td>
    </tr>
    <tr>
      <td>Banana</td>
      <td>$0.50</td>
    </tr>
  </tbody>
</table>
```

**Output Markdown:**
```markdown
| Product | Price |
| :--- | :--- |
| Apple | $1.00 |
| Banana | $0.50 |
```

## Table Structure Detection

The converter handles various HTML table structures:

### Explicit Headers (th)
```html
<table>
  <tr>
    <th>Header 1</th>
    <th>Header 2</th>
  </tr>
  <tr>
    <td>Data 1</td>
    <td>Data 2</td>
  </tr>
</table>
```
✅ Headers detected from `<th>` elements

### First Row as Headers
```html
<table>
  <tr>
    <td>Name</td>
    <td>Age</td>
  </tr>
  <tr>
    <td>Alice</td>
    <td>30</td>
  </tr>
</table>
```
✅ First row treated as headers when no `<th>` elements found

### Semantic Structure
```html
<table>
  <thead>
    <tr><th>...</th></tr>
  </thead>
  <tbody>
    <tr><td>...</td></tr>
  </tbody>
</table>
```
✅ Proper `<thead>` and `<tbody>` recognized

## Content Handling

### Text Extraction
- Extracts text from plain content
- Handles formatted content (bold, italic, etc.)
- Normalizes whitespace
- Removes extra spaces and line breaks

### Special Characters
- Escapes pipe characters (|) in content
- Handles unicode characters
- Preserves spacing and punctuation

### Empty Cells
```html
<tr>
  <td>Value</td>
  <td></td>
  <td>Another</td>
</tr>
```
✅ Empty cells preserved as blank

### Formatted Content
```html
<td><strong>Bold</strong> text</td>
```
✅ Extracts: "Bold text"

## Examples

### Example 1: Data Table

**Input:**
```html
<table>
  <tr>
    <th>Quarter</th>
    <th>Revenue</th>
    <th>Profit</th>
  </tr>
  <tr>
    <td>Q1 2024</td>
    <td>$100,000</td>
    <td>$40,000</td>
  </tr>
  <tr>
    <td>Q2 2024</td>
    <td>$120,000</td>
    <td>$55,000</td>
  </tr>
</table>
```

**Output:**
```markdown
| Quarter | Revenue | Profit |
| :--- | :--- | :--- |
| Q1 2024 | $100,000 | $40,000 |
| Q2 2024 | $120,000 | $55,000 |
```

### Example 2: Comparison Table

**Input:**
```html
<table>
  <tr>
    <th>Feature</th>
    <th>Plan A</th>
    <th>Plan B</th>
  </tr>
  <tr>
    <td>Basic Support</td>
    <td>✓</td>
    <td>✓</td>
  </tr>
  <tr>
    <td>Premium Support</td>
    <td>✗</td>
    <td>✓</td>
  </tr>
</table>
```

**Output:**
```markdown
| Feature | Plan A | Plan B |
| :--- | :--- | :--- |
| Basic Support | ✓ | ✓ |
| Premium Support | ✗ | ✓ |
```

### Example 3: Wide Table

**Input:**
```html
<table>
  <tr>
    <th>ID</th>
    <th>Name</th>
    <th>Email</th>
    <th>Status</th>
  </tr>
  <tr>
    <td>001</td>
    <td>John</td>
    <td>john@example.com</td>
    <td>Active</td>
  </tr>
</table>
```

**Output:**
```markdown
| ID | Name | Email | Status |
| :--- | :--- | :--- | :--- |
| 001 | John | john@example.com | Active |
```

## Column Alignment

By default, all columns are left-aligned:
```markdown
| Left | Left | Left |
| :--- | :--- | :--- |
| 1 | 2 | 3 |
```

### Future Enhancements
- Detect numeric columns (right-align)
- Detect centered content
- Custom alignment configuration

## Multiple Tables

Documents with multiple tables are all converted:

```html
<p>First table:</p>
<table>...</table>

<p>Second table:</p>
<table>...</table>
```

Result: Both tables converted to Markdown format with proper spacing.

## Processing Order

When multiple features are enabled:

1. **Table Conversion** (first)
2. **Image Extraction** (second)
3. **Metadata Extraction** (third)
4. **CSS Hints** (fourth)
5. **TOC Generation** (last)

This order ensures:
- Tables are converted before other processing
- Content structure is preserved
- Features don't interfere with each other

## Integration with Other Features

### With Metadata Extraction
```json
{
  "convert_tables": true,
  "extract_metadata": true
}
```
Output: YAML frontmatter + Markdown tables + content

### With TOC Generation
```json
{
  "convert_tables": true,
  "generate_toc": true
}
```
Output: Table of contents + Markdown tables + content

### With Image Extraction
```json
{
  "convert_tables": true,
  "extract_images": true
}
```
Output: Markdown tables + image references + content

## Performance

Table conversion is highly efficient:
- Linear scan through table elements: O(n)
- Cell text extraction: O(m) per cell
- Markdown generation: O(k) where k = total cell count

Typical times:
- Simple table (10 rows × 3 cols): < 1ms
- Medium table (100 rows × 5 cols): 1-5ms
- Large table (1000 rows × 10 cols): 10-50ms

## Limitations

### Not Supported
- **Colspan/Rowspan** - Merged cells not fully supported (treated as separate)
- **Nested Tables** - Tables within cells may not convert correctly
- **Complex Formatting** - Some CSS styling may be lost
- **Interactive Elements** - Buttons, inputs in tables converted to text
- **Alignment Hints** - HTML alignment attributes not detected
- **Custom Attributes** - Table class/id attributes ignored

### Handled Gracefully
- Empty cells ✓
- Multiple tables ✓
- Mixed th/td elements ✓
- Missing structure ✓
- Special characters in content ✓

## Troubleshooting

### Tables Not Converted

**Issue:** `convert_tables` is `false` by default
**Solution:** Set `convert_tables: true` in parameters

### Table Content Missing

**Issue:** Complex nested HTML in cells
**Solution:** Content is extracted as plain text; formatting is simplified

### Misaligned Columns

**Issue:** Uneven cell counts across rows
**Solution:** Rows automatically padded to match header count

### Special Characters Not Displaying

**Issue:** Unicode or special formatting
**Solution:** Check source HTML encoding; use UTF-8

## Markdown Compatibility

Output is compatible with:
- Standard Markdown ✓
- GitHub Flavored Markdown (GFM) ✓
- CommonMark ✓
- Most static site generators ✓

## Testing

Example file: `examples/table_demo.html`

Test different table types:
- Simple tables with th/td
- Structured tables with thead/tbody
- Tables with formatted content
- Comparison tables
- Wide tables with many columns
- Empty cells and missing data

## References

- [Markdown Table Syntax](https://github.github.com/gfm/#tables-extension-)
- [HTML Table Element Reference](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/table)
- [GitHub Flavored Markdown Tables](https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/organizing-information-with-tables)
- Related: Image extraction, Metadata extraction, TOC generation

