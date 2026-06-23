# Heading Analysis

Analyze document heading structure, validate hierarchy, and identify potential issues.

## Overview

The heading analysis feature examines all headings in HTML documents and validates their hierarchical structure. This is useful for:

- **Document Structure** - Understand document outline and organization
- **Accessibility** - Ensure proper heading hierarchy for screen readers
- **Quality Assurance** - Identify improper heading nesting
- **SEO** - Validate heading structure for search engines
- **Navigation** - Generate automatic table of contents
- **Content Organization** - Analyze document flow and levels

## How It Works

1. **Extract Headings** - Find all h1-h6 elements in document order
2. **Build Hierarchy** - Create parent-child relationships
3. **Analyze Structure** - Check for proper nesting and hierarchy
4. **Report Issues** - Generate summary of any structural problems
5. **Visualize** - Create tree and outline views

## Issues Detected

### Jump in Heading Levels
Improper jumps in heading hierarchy (e.g., h1 directly to h3):
```
❌ Jump from h1 to h3 (missing h2)
```

### Missing H1
Document should start with single h1:
```
❌ Document has no h1 tag
```

### Multiple H1s
Only one h1 should exist per page:
```
❌ Document has 3 h1 tags (should be 1)
```

### Doesn't Start with H1
First heading should be h1:
```
❌ Document should start with h1, but starts with h2
```

## Usage

### Analyze Document Structure

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "analyze_headings": true
  }
}
```

### With Other Features

Combine with other analysis features:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "analyze_headings": true,
    "extract_links": true,
    "preserve_comments": true
  }
}
```

## Output Format

### Input HTML
```html
<!DOCTYPE html>
<html>
<head><title>Page</title></head>
<body>
    <h1>Main Title</h1>
    <h2>Section 1</h2>
    <h3>Subsection</h3>
    <h2>Section 2</h2>
</body>
</html>
```

### Output Analysis

```markdown
## Heading Structure Analysis

**Total Headings:** 4

### Heading Levels Distribution

| Level | Count |
|-------|-------|
| H1 | 1 |
| H2 | 2 |
| H3 | 1 |

**Hierarchy Depth:** 1 - 3

### ✓ No Hierarchy Issues

## Document Heading Structure

```
├─ H1: Main Title
  ├─ H2: Section 1
    ├─ H3: Subsection
  ├─ H2: Section 2
```

# Main Title

...content follows...
```

## Statistics Provided

- **Total Headings** - Count of all h1-h6 tags
- **Level Distribution** - Count of headings per level (h1-h6)
- **Hierarchy Depth** - Minimum and maximum nesting levels
- **Issues Found** - List of any structural problems

## Output Sections

### Heading Structure Analysis
Statistical summary of heading distribution and hierarchy.

### Heading Levels Distribution
Table showing count of each heading level (h1-h6).

### Hierarchy Issues (if any)
- Jump in heading levels
- Multiple h1 tags
- No h1 tag
- Document doesn't start with h1
- Broken nesting

### Document Heading Structure
Visual tree showing hierarchical organization.

## Examples

### Well-Structured Document

**HTML:**
```html
<h1>Product Documentation</h1>
<h2>Installation</h2>
<h3>Prerequisites</h3>
<h3>Setup Steps</h3>
<h2>Usage</h2>
<h3>Configuration</h3>
<h3>Examples</h3>
<h2>Troubleshooting</h2>
```

**Analysis Output:**
```
✓ No Hierarchy Issues

Total Headings: 8
Hierarchy Depth: 1 - 3
Distribution: H1=1, H2=3, H3=4
```

### Problematic Document

**HTML:**
```html
<h2>Getting Started</h2>
<h1>Title</h1>
<h3>Details</h3>
<h1>Another Title</h1>
```

**Analysis Output:**
```
⚠️ Hierarchy Issues Found:

1. Document should start with h1, but starts with h2
2. Jump from h1 to h3 (missing h2)
3. Document has 2 h1 tags (should be 1)
```

## Processing Order

When combined with other features:
1. **Heading Analysis** (first)
2. **Link Extraction**
3. **Comment Preservation**
4. **Form Extraction**
5. **Table Conversion**
6. **Image Extraction**
7. **HTML to Markdown**

## Implementation Details

### Module Structure

```rust
pub struct Heading {
    pub level: usize,
    pub text: String,
    pub id: Option<String>,
    pub parent_levels: Vec<usize>,
}

pub struct HeadingStatistics {
    pub total_headings: usize,
    pub levels_count: BTreeMap<usize, usize>,
    pub max_depth: usize,
    pub min_depth: usize,
    pub has_hierarchy_issues: bool,
    pub issues: Vec<String>,
}

pub fn extract_headings_from_html(html: &str) -> Result<Vec<Heading>>
pub fn analyze_heading_structure(headings: &[Heading]) -> HeadingStatistics
pub fn generate_heading_tree(headings: &[Heading]) -> String
```

### Hierarchy Building

Headings are extracted in document order (not by level), preserving the structure. Parent levels are computed based on heading nesting.

## Best Practices

### Proper Heading Hierarchy
```
✓ h1 (one per page)
  ✓ h2 (main sections)
    ✓ h3 (subsections)
      ✓ h4 (sub-subsections)
```

### Avoid These Issues
```
✗ Starting with h2 instead of h1
✗ h1 -> h3 (skipping h2)
✗ Multiple h1 tags
✗ h6 -> h1 (wrong order)
✗ Using headings for styling instead of structure
```

## Accessibility Impact

Proper heading hierarchy is crucial for:
- **Screen Readers** - Navigate document structure
- **Keyboard Users** - Jump between sections
- **Search Engines** - Understand page structure
- **Cognitive Users** - Understand document organization

## Testing

Heading analysis works with any HTML file containing headings:

```bash
# Test with any HTML file
cargo build --release
echo '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"test.html","analyze_headings":true}}}' | ./target/release/to_markdown_mcp
```

## Integration with Other Features

### With Link Extraction
Heading analysis runs first, providing structure before link analysis.

### With Form Extraction
Headings analyzed independently of forms in the document.

### With Table Conversion
Heading structure preserved separately from table conversion.

### With Metadata Extraction
Headings provide document organization independent of metadata.

## Limitations

- **Heading Text** - Only visible text extracted (styled/formatted preserved)
- **Heading IDs** - Currently optional (some pages don't include them)
- **Structure Only** - Analysis focuses on hierarchy, not heading quality
- **No Content Analysis** - Doesn't validate if headings match content

## Future Enhancements

Planned improvements:
- Heading quality scoring (length, clarity)
- Heading-content mismatch detection
- Automatic ID generation for headings
- Keyword extraction from headings
- Heading-based search engine optimization
- Accessibility scoring
- Document outline generation
- Structure-based content navigation

## References

- [Web Content Accessibility Guidelines (WCAG)](https://www.w3.org/WAI/WCAG21/quickref/#info-and-relationships)
- [HTML Heading Elements](https://html.spec.whatwg.org/#heading-content)
- [Screen Reader Navigation](https://www.boia.org/blog/how-do-screen-readers-read-a-website)
- Related: Link extraction, Comment preservation, Form extraction
