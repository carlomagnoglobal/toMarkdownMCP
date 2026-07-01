use anyhow::{anyhow, Result};
use scraper::{Html, Selector};

/// HTML form field type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Password,
    Email,
    Number,
    Tel,
    Url,
    Date,
    Time,
    Checkbox,
    Radio,
    Select,
    Textarea,
    Hidden,
    Button,
    Submit,
    Reset,
    File,
    Color,
    Range,
}

impl FieldType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::Text => "text",
            FieldType::Password => "password",
            FieldType::Email => "email",
            FieldType::Number => "number",
            FieldType::Tel => "tel",
            FieldType::Url => "url",
            FieldType::Date => "date",
            FieldType::Time => "time",
            FieldType::Checkbox => "checkbox",
            FieldType::Radio => "radio",
            FieldType::Select => "select",
            FieldType::Textarea => "textarea",
            FieldType::Hidden => "hidden",
            FieldType::Button => "button",
            FieldType::Submit => "submit",
            FieldType::Reset => "reset",
            FieldType::File => "file",
            FieldType::Color => "color",
            FieldType::Range => "range",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "password" => FieldType::Password,
            "email" => FieldType::Email,
            "number" => FieldType::Number,
            "tel" => FieldType::Tel,
            "url" => FieldType::Url,
            "date" => FieldType::Date,
            "time" => FieldType::Time,
            "checkbox" => FieldType::Checkbox,
            "radio" => FieldType::Radio,
            "select" => FieldType::Select,
            "textarea" => FieldType::Textarea,
            "hidden" => FieldType::Hidden,
            "button" => FieldType::Button,
            "submit" => FieldType::Submit,
            "reset" => FieldType::Reset,
            "file" => FieldType::File,
            "color" => FieldType::Color,
            "range" => FieldType::Range,
            _ => FieldType::Text,
        }
    }
}

/// Represents a form input field
#[derive(Debug, Clone)]
pub struct FormField {
    pub name: String,
    pub label: Option<String>,
    pub field_type: FieldType,
    pub required: bool,
    pub placeholder: Option<String>,
    pub value: Option<String>,
    pub options: Vec<(String, String)>, // (value, label) for select/radio/checkbox
}

/// Represents an HTML form
#[derive(Debug, Clone)]
pub struct Form {
    pub id: Option<String>,
    pub method: String,           // GET, POST, etc.
    pub action: Option<String>,   // Form submission target
    pub enctype: Option<String>,  // Encoding type
    pub fields: Vec<FormField>,
}

/// Extract all forms from HTML content
pub fn extract_forms_from_html(html_content: &str) -> Result<Vec<Form>> {
    let document = Html::parse_document(html_content);
    let mut forms = Vec::new();

    let form_selector = Selector::parse("form")
        .map_err(|_| anyhow!("Invalid selector for form"))?;

    for form_elem in document.select(&form_selector) {
        if let Ok(form) = parse_html_form(&form_elem) {
            forms.push(form);
        }
        // Continue on error to be resilient
    }

    Ok(forms)
}

/// Parse a single HTML form element
pub fn parse_html_form(form_elem: &scraper::element_ref::ElementRef) -> Result<Form> {
    let id = form_elem.value().attr("id").map(|s| s.to_string());
    let method = form_elem
        .value()
        .attr("method")
        .unwrap_or("GET")
        .to_uppercase();
    let action = form_elem.value().attr("action").map(|s| s.to_string());
    let enctype = form_elem.value().attr("enctype").map(|s| s.to_string());

    let mut fields = Vec::new();

    // Extract input fields
    let input_selector = Selector::parse("input")
        .map_err(|_| anyhow!("Invalid selector"))?;
    for input_elem in form_elem.select(&input_selector) {
        if let Some(field) = extract_input_field(&input_elem, form_elem) {
            fields.push(field);
        }
    }

    // Extract textarea fields
    let textarea_selector = Selector::parse("textarea")
        .map_err(|_| anyhow!("Invalid selector"))?;
    for textarea_elem in form_elem.select(&textarea_selector) {
        if let Some(field) = extract_textarea_field(&textarea_elem, form_elem) {
            fields.push(field);
        }
    }

    // Extract select fields
    let select_selector = Selector::parse("select")
        .map_err(|_| anyhow!("Invalid selector"))?;
    for select_elem in form_elem.select(&select_selector) {
        if let Some(field) = extract_select_field(&select_elem, form_elem) {
            fields.push(field);
        }
    }

    // Extract button fields
    let button_selector = Selector::parse("button")
        .map_err(|_| anyhow!("Invalid selector"))?;
    for button_elem in form_elem.select(&button_selector) {
        if let Some(field) = extract_button_field(&button_elem, form_elem) {
            fields.push(field);
        }
    }

    Ok(Form {
        id,
        method,
        action,
        enctype,
        fields,
    })
}

/// Extract label for a field
fn get_field_label(
    field_elem: &scraper::element_ref::ElementRef,
    form_elem: &scraper::element_ref::ElementRef,
) -> Option<String> {
    // First try to get label from associated <label> element
    if let Some(field_id) = field_elem.value().attr("id") {
        let label_selector = Selector::parse(&format!("label[for='{}']", field_id)).ok()?;
        if let Some(label_elem) = form_elem.select(&label_selector).next() {
            let text = label_elem.inner_html();
            return Some(text.trim().to_string());
        }
    }

    // Check if field has a title attribute
    if let Some(title) = field_elem.value().attr("title") {
        return Some(title.to_string());
    }

    // Check for aria-label
    if let Some(aria_label) = field_elem.value().attr("aria-label") {
        return Some(aria_label.to_string());
    }

    None
}

/// Extract an input field
fn extract_input_field(
    input_elem: &scraper::element_ref::ElementRef,
    form_elem: &scraper::element_ref::ElementRef,
) -> Option<FormField> {
    let name = input_elem.value().attr("name")?;
    if name.is_empty() {
        return None;
    }

    let input_type = input_elem
        .value()
        .attr("type")
        .unwrap_or("text");
    let field_type = FieldType::from_str(input_type);

    // Skip hidden fields unless explicitly extracted
    if field_type == FieldType::Hidden {
        return None;
    }

    let label = get_field_label(input_elem, form_elem);
    let required = input_elem.value().attr("required").is_some();
    let placeholder = input_elem.value().attr("placeholder").map(|s| s.to_string());
    let value = input_elem.value().attr("value").map(|s| s.to_string());

    Some(FormField {
        name: name.to_string(),
        label,
        field_type,
        required,
        placeholder,
        value,
        options: Vec::new(),
    })
}

/// Extract a textarea field
fn extract_textarea_field(
    textarea_elem: &scraper::element_ref::ElementRef,
    form_elem: &scraper::element_ref::ElementRef,
) -> Option<FormField> {
    let name = textarea_elem.value().attr("name")?;
    if name.is_empty() {
        return None;
    }

    let label = get_field_label(textarea_elem, form_elem);
    let required = textarea_elem.value().attr("required").is_some();
    let placeholder = textarea_elem.value().attr("placeholder").map(|s| s.to_string());
    let value = textarea_elem.inner_html();
    let value = if value.is_empty() {
        None
    } else {
        Some(value.trim().to_string())
    };

    Some(FormField {
        name: name.to_string(),
        label,
        field_type: FieldType::Textarea,
        required,
        placeholder,
        value,
        options: Vec::new(),
    })
}

/// Extract a select field
fn extract_select_field(
    select_elem: &scraper::element_ref::ElementRef,
    form_elem: &scraper::element_ref::ElementRef,
) -> Option<FormField> {
    let name = select_elem.value().attr("name")?;
    if name.is_empty() {
        return None;
    }

    let label = get_field_label(select_elem, form_elem);
    let required = select_elem.value().attr("required").is_some();

    // Extract options
    let mut options = Vec::new();
    let option_selector = Selector::parse("option").ok()?;

    for option_elem in select_elem.select(&option_selector) {
        let opt_value = if let Some(val) = option_elem.value().attr("value") {
            val.to_string()
        } else {
            option_elem.inner_html().trim().to_string()
        };
        let opt_label = option_elem.inner_html();
        let opt_label = opt_label.trim().to_string();

        if !opt_value.is_empty() || !opt_label.is_empty() {
            options.push((opt_value, opt_label));
        }
    }

    Some(FormField {
        name: name.to_string(),
        label,
        field_type: FieldType::Select,
        required,
        placeholder: None,
        value: None,
        options,
    })
}

/// Extract a button field
fn extract_button_field(
    button_elem: &scraper::element_ref::ElementRef,
    _form_elem: &scraper::element_ref::ElementRef,
) -> Option<FormField> {
    let name = button_elem.value().attr("name");
    let button_type = button_elem
        .value()
        .attr("type")
        .unwrap_or("button");
    let field_type = FieldType::from_str(button_type);

    let label = button_elem.inner_html();
    let label = label.trim().to_string();

    Some(FormField {
        name: name.unwrap_or("").to_string(),
        label: Some(label),
        field_type,
        required: false,
        placeholder: None,
        value: None,
        options: Vec::new(),
    })
}

/// Generate Markdown representation of a form
pub fn form_to_markdown(form: &Form) -> String {
    let mut markdown = String::new();

    // Form header
    markdown.push_str("### Form\n\n");

    // Form attributes
    if let Some(ref action) = form.action {
        markdown.push_str(&format!("**Action:** {}\n\n", action));
    }
    if form.method != "GET" {
        markdown.push_str(&format!("**Method:** {}\n\n", form.method));
    }
    if let Some(ref enctype) = form.enctype {
        markdown.push_str(&format!("**Encoding:** {}\n\n", enctype));
    }

    // Fields table
    if !form.fields.is_empty() {
        markdown.push_str("| Field | Type | Required | Details |\n");
        markdown.push_str("|-------|------|----------|----------|\n");

        for field in &form.fields {
            let field_name = field.name.replace('|', "\\|");
            let field_type = field.field_type.as_str();
            let required = if field.required { "✓" } else { "" };

            let mut details = String::new();
            if let Some(ref label) = field.label {
                details.push_str(&format!("Label: *{}*", label.replace('|', "\\|")));
            }
            if let Some(ref placeholder) = field.placeholder {
                if !details.is_empty() {
                    details.push_str(" | ");
                }
                details.push_str(&format!("Placeholder: {}", placeholder.replace('|', "\\|")));
            }
            if !field.options.is_empty() {
                if !details.is_empty() {
                    details.push_str(" | ");
                }
                let opts = field
                    .options
                    .iter()
                    .map(|(_, label)| label.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                details.push_str(&format!("Options: {}", opts));
            }

            markdown.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                field_name, field_type, required, details
            ));
        }
    } else {
        markdown.push_str("*No form fields found*\n");
    }

    markdown.push('\n');
    markdown
}

/// Convert forms in HTML to Markdown and remove/replace form elements
pub fn process_forms_in_html(html_content: &str) -> Result<(String, Vec<Form>)> {
    let forms = extract_forms_from_html(html_content)?;

    if forms.is_empty() {
        return Ok((html_content.to_string(), forms));
    }

    let mut result = html_content.to_string();
    let _document = Html::parse_document(&result);
    let _form_selector = Selector::parse("form")
        .map_err(|_| anyhow!("Invalid selector"))?;

    // Replace forms with markdown-safe div wrappers
    for form in &forms {
        let markdown = form_to_markdown(form);

        // Find and replace the first form tag with a div containing the markdown
        let div_replacement = format!(
            "<div data-form-markdown=\"true\">{}</div>",
            markdown.replace('\n', "<br/>")
        );

        // This is a simplified replacement - in production might need more careful handling
        if let Some(form_start) = result.find("<form") {
            if let Some(form_end) = result[form_start..].find("</form>") {
                let actual_end = form_start + form_end + 7; // +7 for "</form>"
                result.replace_range(form_start..actual_end, &div_replacement);
            }
        }
    }

    Ok((result, forms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_text_input() {
        let html = r#"
            <form>
                <input type="text" name="username" placeholder="Enter username">
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        assert_eq!(forms.len(), 1);
        assert_eq!(forms[0].fields.len(), 1);
        assert_eq!(forms[0].fields[0].name, "username");
        assert_eq!(forms[0].fields[0].field_type, FieldType::Text);
        assert_eq!(forms[0].fields[0].placeholder, Some("Enter username".to_string()));
    }

    #[test]
    fn test_extract_email_input() {
        let html = r#"
            <form>
                <input type="email" name="email" required>
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        assert_eq!(forms[0].fields[0].field_type, FieldType::Email);
        assert!(forms[0].fields[0].required);
    }

    #[test]
    fn test_extract_textarea() {
        let html = r#"
            <form>
                <textarea name="comments" placeholder="Your comments"></textarea>
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        assert_eq!(forms[0].fields[0].field_type, FieldType::Textarea);
        assert_eq!(forms[0].fields[0].name, "comments");
    }

    #[test]
    fn test_extract_select() {
        let html = r#"
            <form>
                <select name="country">
                    <option value="us">United States</option>
                    <option value="uk">United Kingdom</option>
                </select>
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        let field = &forms[0].fields[0];
        assert_eq!(field.field_type, FieldType::Select);
        assert_eq!(field.options.len(), 2);
        assert_eq!(field.options[0], ("us".to_string(), "United States".to_string()));
    }

    #[test]
    fn test_extract_checkbox() {
        let html = r#"
            <form>
                <input type="checkbox" name="agree" value="yes">
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        assert_eq!(forms[0].fields[0].field_type, FieldType::Checkbox);
    }

    #[test]
    fn test_extract_radio() {
        let html = r#"
            <form>
                <input type="radio" name="gender" value="m">
                <input type="radio" name="gender" value="f">
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        assert_eq!(forms[0].fields.len(), 2);
        assert_eq!(forms[0].fields[0].field_type, FieldType::Radio);
    }

    #[test]
    fn test_form_attributes() {
        let html = r#"
            <form id="contact-form" method="POST" action="/submit" enctype="multipart/form-data">
                <input type="text" name="name">
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        assert_eq!(forms[0].id, Some("contact-form".to_string()));
        assert_eq!(forms[0].method, "POST");
        assert_eq!(forms[0].action, Some("/submit".to_string()));
        assert_eq!(forms[0].enctype, Some("multipart/form-data".to_string()));
    }

    #[test]
    fn test_form_to_markdown() {
        let form = Form {
            id: None,
            method: "POST".to_string(),
            action: Some("/submit".to_string()),
            enctype: None,
            fields: vec![
                FormField {
                    name: "username".to_string(),
                    label: Some("Username".to_string()),
                    field_type: FieldType::Text,
                    required: true,
                    placeholder: Some("Enter username".to_string()),
                    value: None,
                    options: Vec::new(),
                },
                FormField {
                    name: "country".to_string(),
                    label: None,
                    field_type: FieldType::Select,
                    required: false,
                    placeholder: None,
                    value: None,
                    options: vec![
                        ("us".to_string(), "United States".to_string()),
                        ("uk".to_string(), "United Kingdom".to_string()),
                    ],
                },
            ],
        };

        let markdown = form_to_markdown(&form);
        assert!(markdown.contains("### Form"));
        assert!(markdown.contains("/submit"));
        assert!(markdown.contains("POST"));
        assert!(markdown.contains("username"));
        assert!(markdown.contains("Username"));
    }

    #[test]
    fn test_multiple_forms() {
        let html = r#"
            <form>
                <input type="text" name="field1">
            </form>
            <form>
                <input type="email" name="field2">
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        assert_eq!(forms.len(), 2);
        assert_eq!(forms[0].fields[0].name, "field1");
        assert_eq!(forms[1].fields[0].name, "field2");
    }

    #[test]
    fn test_field_type_parsing() {
        assert_eq!(FieldType::from_str("email"), FieldType::Email);
        assert_eq!(FieldType::from_str("password"), FieldType::Password);
        assert_eq!(FieldType::from_str("invalid"), FieldType::Text);
        assert_eq!(FieldType::from_str("NUMBER"), FieldType::Number);
    }

    #[test]
    fn test_extract_button() {
        let html = r#"
            <form>
                <button type="submit">Submit Form</button>
            </form>
        "#;

        let forms = extract_forms_from_html(html).unwrap();
        assert_eq!(forms[0].fields[0].field_type, FieldType::Submit);
        assert_eq!(forms[0].fields[0].label, Some("Submit Form".to_string()));
    }
}
