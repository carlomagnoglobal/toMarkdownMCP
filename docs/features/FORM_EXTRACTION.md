# Form Extraction

Extract HTML forms and convert them to readable Markdown tables for documentation and content migration.

## Overview

The form extraction feature analyzes HTML forms and converts them to Markdown, preserving all field information in an easy-to-read table format. This is useful for:

- **Documentation** - Document form requirements in Markdown
- **Content Migration** - Convert web forms to documentation
- **Accessibility** - Create accessible text representations of forms
- **Archival** - Preserve form structure for records or compliance
- **Testing** - Generate form specifications from HTML

## How It Works

1. **Scan HTML** - Find all `<form>` elements in the HTML
2. **Extract Fields** - Parse input, textarea, select, and button elements
3. **Gather Metadata** - Collect field attributes (name, type, required, etc.)
4. **Generate Table** - Convert to Markdown table with all field details
5. **Replace Forms** - Replace form elements with Markdown content

## Supported Form Elements

### Input Types
- **Text Input** - Single-line text fields
- **Email** - Email validation input
- **Password** - Masked password input
- **Number** - Numeric input with validation
- **Tel** - Telephone number input
- **URL** - URL validation input
- **Date** - Date picker
- **Time** - Time picker
- **Checkbox** - Boolean checkbox field
- **Radio** - Radio button (grouped by name)
- **File** - File upload field
- **Color** - Color picker
- **Range** - Slider input

### Other Elements
- **Textarea** - Multi-line text area
- **Select** - Dropdown selection with options
- **Button** - Submit, reset, or action buttons
- **Labels** - Associated field labels

### Form Attributes
- **Method** - GET, POST, etc.
- **Action** - Form submission URL
- **Enctype** - Encoding type (e.g., multipart/form-data)
- **ID** - Form identifier

## Usage

### Basic Extraction

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "forms.html",
    "extract_forms": true
  }
}
```

### With Other Features

Combine form extraction with other features:

```json
{
  "name": "convert_file",
  "arguments": {
    "file_path": "page.html",
    "extract_forms": true,
    "extract_metadata": true,
    "extract_images": true,
    "convert_tables": true
  }
}
```

## Output Format

### Input HTML
```html
<form method="POST" action="/contact">
    <label for="name">Name:</label>
    <input type="text" id="name" name="name" required placeholder="Your name">
    
    <label for="email">Email:</label>
    <input type="email" id="email" name="email" required>
    
    <label for="country">Country:</label>
    <select id="country" name="country">
        <option value="us">United States</option>
        <option value="uk">United Kingdom</option>
    </select>
    
    <button type="submit">Send</button>
</form>
```

### Output Markdown
```markdown
### Form

**Action:** /contact

**Method:** POST

| Field | Type | Required | Details |
|-------|------|----------|---------|
| name | text | ✓ | Label: *Name* \| Placeholder: Your name |
| email | email | ✓ | Label: *Email* |
| country | select | | Options: United States, United Kingdom |
| | submit | | Label: Send |
```

## Field Information Captured

Each form field in the Markdown table includes:

- **Field Name** - The input's `name` attribute
- **Type** - Input type (text, email, select, etc.)
- **Required** - Whether field is required (✓ or blank)
- **Details** - Label, placeholder, or available options

For select dropdowns, all option labels are listed.

For radio buttons and checkboxes, the name is repeated for each variant.

## Examples

### Contact Form

**HTML:**
```html
<form id="contact" method="POST" action="/submit">
    <input type="text" name="name" placeholder="Full Name" required>
    <input type="email" name="email" required>
    <textarea name="message" placeholder="Your message"></textarea>
    <button type="submit">Send</button>
</form>
```

**Markdown Output:**
```markdown
### Form

**Action:** /submit

**Method:** POST

| Field | Type | Required | Details |
|-------|------|----------|---------|
| name | text | ✓ | Placeholder: Full Name |
| email | email | ✓ | |
| message | textarea | | Placeholder: Your message |
| | submit | | Label: Send |
```

### Registration Form

**HTML:**
```html
<form method="POST" action="/register">
    <label for="username">Username</label>
    <input type="text" id="username" name="username" required>
    
    <label for="password">Password</label>
    <input type="password" id="password" name="password" required>
    
    <label for="age">Age</label>
    <input type="number" id="age" name="age" min="18">
    
    <label>
        <input type="checkbox" name="terms" required>
        I agree to terms
    </label>
    
    <button type="submit">Register</button>
</form>
```

**Markdown Output:**
```markdown
### Form

**Method:** POST

**Action:** /register

| Field | Type | Required | Details |
|-------|------|----------|---------|
| username | text | ✓ | Label: *Username* |
| password | password | ✓ | Label: *Password* |
| age | number | | Label: *Age* |
| terms | checkbox | ✓ | Label: *I agree to terms* |
| | submit | | Label: Register |
```

### Survey Form with Radio Buttons

**HTML:**
```html
<form method="POST">
    <label>How satisfied are you?</label>
    <label>
        <input type="radio" name="satisfaction" value="5">
        Very Satisfied
    </label>
    <label>
        <input type="radio" name="satisfaction" value="4">
        Satisfied
    </label>
    <label>
        <input type="radio" name="satisfaction" value="3">
        Neutral
    </label>
</form>
```

**Markdown Output:**
```markdown
### Form

**Method:** GET

| Field | Type | Required | Details |
|-------|------|----------|---------|
| satisfaction | radio | | |
| satisfaction | radio | | |
| satisfaction | radio | | |
```

## Label Detection

Form labels are identified through multiple methods, in priority order:

1. **Associated `<label>` with `for` attribute** - Most reliable
2. **`title` attribute** - Fallback if no label element
3. **`aria-label` attribute** - Accessibility label
4. **Button text** - For button elements

### Example

```html
<label for="email">Enter your email:</label>
<input id="email" type="email" name="user_email">

<!-- vs -->

<input type="email" name="user_email" placeholder="email@example.com">
```

Both are captured, though the first provides better labeling in output.

## Implementation Details

### Module Structure

```rust
pub enum FieldType { Text, Email, Password, Number, ... }
pub struct FormField { name, label, field_type, required, ... }
pub struct Form { id, method, action, enctype, fields }

pub fn extract_forms_from_html(html: &str) -> Result<Vec<Form>>
pub fn form_to_markdown(form: &Form) -> String
pub fn process_forms_in_html(html: &str) -> Result<(String, Vec<Form>)>
```

### Processing Order

When combined with other features:
1. **Form Extraction** (first - modifies HTML)
2. **Table Conversion** (processes remaining HTML tables)
3. **Image Extraction** (processes remaining images)
4. **HTML to Markdown** (final conversion)
5. **Metadata Extraction** (post-processing)
6. **TOC Generation** (final)

## Limitations

- **Hidden fields** - Skipped (id="hidden" inputs)
- **Complex nested structures** - Simplified in table format
- **Form validation rules** - Not captured (min, max, pattern, etc.)
- **Styling** - Not preserved (CSS classes, inline styles)
- **Form scripts** - Not converted (JavaScript handlers)
- **Fieldset grouping** - Not explicitly shown (all fields in one table)

## Capabilities vs Limitations

### What Works Well
- Simple contact/signup forms ✅
- Registration and login forms ✅
- Survey and feedback forms ✅
- Product order forms ✅
- Search and filter forms ✅

### What Has Limitations
- Forms with complex JavaScript validation
- Dynamically generated form fields
- Multi-step wizard forms
- Forms with custom input types
- Heavily styled forms where appearance matters

## Testing

Example test file: `examples/form_demo.html`

Contains:
- Contact form (POST)
- Registration form (multipart)
- Product inquiry form (radios/checkboxes)
- Feedback form (textarea + select)
- Advanced input types
- Minimal search form

Test with:
```bash
# Extract forms from example
cargo run < examples/form_demo.html

# Convert file with form extraction
cargo build --release
./target/release/to_markdown_mcp <<EOF
{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"convert_file","arguments":{"file_path":"examples/form_demo.html","extract_forms":true}}}
EOF
```

## Integration with Other Features

### With Table Conversion
Forms are extracted first, then remaining HTML tables are converted.

### With Image Extraction
Forms don't contain images, so both work independently.

### With Metadata Extraction
Form extraction happens before metadata extraction - no conflicts.

### With Code Language Detection
Forms don't contain code blocks - independent features.

### With CSS Hints
Form styling is not preserved (forms are structure-only).

## Future Enhancements

Planned improvements:
- Preserve form validation attributes (min, max, pattern)
- Support for input groups and fieldsets
- Custom field descriptions from aria-description
- Conditional field dependencies
- Form event handlers documentation
- Multi-step form support
- YAML/JSON schema export option
- Form API endpoint documentation

## References

- [HTML Forms Spec](https://html.spec.whatwg.org/multipage/forms.html)
- [Markdown Tables](https://www.markdownguide.org/extended-syntax/#tables)
- [Form Accessibility](https://www.w3.org/WAI/tutorials/forms/)
- Related: Image extraction, Table conversion, Metadata extraction
