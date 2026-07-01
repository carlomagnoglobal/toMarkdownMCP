use anyhow::{anyhow, Result};
use scraper::{Html, Selector};

/// Represents a markdown table
#[derive(Debug, Clone)]
pub struct MarkdownTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub align: Vec<TableAlignment>,
}

/// Column alignment for markdown tables
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    Left,
    Center,
    Right,
}

impl TableAlignment {
    /// Convert alignment to markdown format
    pub fn to_markdown(&self) -> &'static str {
        match self {
            TableAlignment::Left => ":---",
            TableAlignment::Center => ":---:",
            TableAlignment::Right => "---:",
        }
    }
}

/// Extract all tables from HTML
pub fn extract_tables_from_html(html_content: &str) -> Result<Vec<MarkdownTable>> {
    let document = Html::parse_document(html_content);
    let mut tables = Vec::new();

    let table_selector = Selector::parse("table")
        .map_err(|_| anyhow!("Invalid selector for table"))?;

    for table_elem in document.select(&table_selector) {
        if let Ok(table) = parse_html_table(&table_elem) {
            tables.push(table);
        }
        // Continue on error to be resilient
    }

    Ok(tables)
}

/// Parse a single HTML table element
pub fn parse_html_table(table_elem: &scraper::element_ref::ElementRef) -> Result<MarkdownTable> {
    let tr_selector = Selector::parse("tr").map_err(|_| anyhow!("Invalid selector"))?;
    let th_selector = Selector::parse("th").map_err(|_| anyhow!("Invalid selector"))?;
    let td_selector = Selector::parse("td").map_err(|_| anyhow!("Invalid selector"))?;

    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let align: Vec<TableAlignment>;
    let mut first_data_row = true;

    // Process all rows
    for row_elem in table_elem.select(&tr_selector) {
        // Try to extract th elements first (headers)
        let mut th_cells = Vec::new();
        for th in row_elem.select(&th_selector) {
            th_cells.push(extract_cell_text(&th));
        }

        // Try to extract td elements
        let mut td_cells = Vec::new();
        for td in row_elem.select(&td_selector) {
            td_cells.push(extract_cell_text(&td));
        }

        // Decide what this row is
        if !th_cells.is_empty() {
            // This row has th elements - it's a header row
            if headers.is_empty() {
                headers = th_cells;
            }
        } else if !td_cells.is_empty() {
            // This row has td elements
            if headers.is_empty() && first_data_row {
                // No headers found yet, treat first data row as headers
                headers = td_cells;
                first_data_row = false;
            } else {
                // This is a data row
                rows.push(td_cells);
                first_data_row = false;
            }
        }
    }

    if headers.is_empty() && rows.is_empty() {
        return Err(anyhow!("Empty table"));
    }

    // If we have no headers but have rows, create empty headers
    if headers.is_empty() && !rows.is_empty() {
        headers = vec!["".to_string(); rows[0].len()];
    }

    // Initialize alignment (default to left) - all columns left-aligned by default
    align = vec![TableAlignment::Left; headers.len()];

    // Pad rows to match header count
    for row in &mut rows {
        while row.len() < headers.len() {
            row.push(String::new());
        }
        row.truncate(headers.len());
    }

    Ok(MarkdownTable {
        headers,
        rows,
        align,
    })
}

/// Extract text content from a table cell
fn extract_cell_text(cell: &scraper::element_ref::ElementRef) -> String {
    let mut result = String::new();

    // Recursively extract all text from this cell
    for child in cell.children() {
        if let Some(text_node) = child.value().as_text() {
            result.push_str(text_node.trim());
        } else if let Some(elem_ref) = scraper::element_ref::ElementRef::wrap(child) {
            result.push_str(&extract_text_recursive(&elem_ref));
        }
    }

    // Clean up: normalize whitespace
    let cleaned = result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Escape pipe characters in cell content
    cleaned.replace('|', "\\|")
}

/// Recursively extract text from element and children
fn extract_text_recursive(elem: &scraper::element_ref::ElementRef) -> String {
    let tag_name = elem.value().name();
    let mut text = String::new();

    // Special handling for <br>
    if tag_name == "br" {
        return " ".to_string();
    }

    // Get all text nodes in this element
    for child in elem.children() {
        if let Some(text_node) = child.value().as_text() {
            text.push_str(text_node);
        } else if let Some(child_elem) = scraper::element_ref::ElementRef::wrap(child) {
            text.push_str(&extract_text_recursive(&child_elem));
        }
    }

    text
}

/// Convert a table to markdown format
pub fn table_to_markdown(table: &MarkdownTable) -> String {
    let mut result = String::new();

    // Write headers
    result.push('|');
    for header in &table.headers {
        result.push(' ');
        result.push_str(header);
        result.push_str(" |");
    }
    result.push('\n');

    // Write separator
    result.push('|');
    for align in &table.align {
        result.push(' ');
        result.push_str(align.to_markdown());
        result.push_str(" |");
    }
    result.push('\n');

    // Write rows
    for row in &table.rows {
        result.push('|');
        for cell in row {
            result.push(' ');
            result.push_str(cell);
            result.push_str(" |");
        }
        result.push('\n');
    }

    result
}

/// Convert HTML tables in content to markdown
pub fn convert_tables_in_html(html_content: &str) -> Result<String> {
    let tables = extract_tables_from_html(html_content)?;

    if tables.is_empty() {
        return Ok(html_content.to_string());
    }

    let document = Html::parse_document(html_content);
    let table_selector = Selector::parse("table").map_err(|_| anyhow!("Invalid selector"))?;

    let mut result = html_content.to_string();
    let mut table_index = 0;

    // Process each table - note: this is a simplified approach
    // For production use, a more sophisticated approach would preserve HTML structure
    for _table in document.select(&table_selector) {
        if table_index < tables.len() {
            let markdown_table = table_to_markdown(&tables[table_index]);
            // Replace first <table>...</table> with markdown
            if let Some(start) = result.find("<table") {
                if let Some(end) = result[start..].find("</table>") {
                    let end_pos = start + end + 8; // 8 = length of "</table>"
                    result.replace_range(start..end_pos, &markdown_table);
                }
            }
            table_index += 1;
        }
    }

    Ok(result)
}

/// Get information about tables in HTML
pub fn get_table_info(html_content: &str) -> Result<String> {
    let tables = extract_tables_from_html(html_content)?;

    let mut info = String::new();
    info.push_str("# Tables Found\n\n");
    info.push_str(&format!("Total tables: {}\n\n", tables.len()));

    for (idx, table) in tables.iter().enumerate() {
        info.push_str(&format!("## Table {}\n", idx + 1));
        info.push_str(&format!("- Columns: {}\n", table.headers.len()));
        info.push_str(&format!("- Rows: {}\n", table.rows.len()));
        info.push_str(&format!("- Headers: {}\n\n", table.headers.join(", ")));
    }

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_table() {
        let html = r#"
            <table>
                <tr><th>Name</th><th>Age</th></tr>
                <tr><td>Alice</td><td>30</td></tr>
                <tr><td>Bob</td><td>25</td></tr>
            </table>
        "#;

        let tables = extract_tables_from_html(html).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].headers, vec!["Name", "Age"]);
        assert_eq!(tables[0].rows.len(), 2);
    }

    #[test]
    fn test_table_with_thead_tbody() {
        let html = r#"
            <table>
                <thead>
                    <tr><th>Product</th><th>Price</th></tr>
                </thead>
                <tbody>
                    <tr><td>Apple</td><td>$1</td></tr>
                    <tr><td>Banana</td><td>$0.50</td></tr>
                </tbody>
            </table>
        "#;

        let tables = extract_tables_from_html(html).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].headers, vec!["Product", "Price"]);
        assert_eq!(tables[0].rows.len(), 2);
    }

    #[test]
    fn test_table_with_formatted_cells() {
        let html = r#"
            <table>
                <tr><th><strong>Bold</strong></th><th><em>Italic</em></th></tr>
                <tr><td>Regular</td><td>Text</td></tr>
            </table>
        "#;

        let tables = extract_tables_from_html(html).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].rows[0].len(), 2);
    }

    #[test]
    fn test_table_with_pipe_characters() {
        let html = r#"
            <table>
                <tr><th>Data</th></tr>
                <tr><td>Value|With|Pipes</td></tr>
            </table>
        "#;

        let tables = extract_tables_from_html(html).unwrap();
        // Pipes should be escaped
        assert!(tables[0].rows[0][0].contains("\\|"));
    }

    #[test]
    fn test_table_to_markdown_format() {
        let table = MarkdownTable {
            headers: vec!["Name".to_string(), "Age".to_string()],
            rows: vec![
                vec!["Alice".to_string(), "30".to_string()],
                vec!["Bob".to_string(), "25".to_string()],
            ],
            align: vec![TableAlignment::Left, TableAlignment::Left],
        };

        let markdown = table_to_markdown(&table);
        assert!(markdown.contains("| Name"));
        assert!(markdown.contains("| Age"));
        assert!(markdown.contains(":---"));
        assert!(markdown.contains("| Alice"));
        assert!(markdown.contains("| Bob"));
    }

    #[test]
    fn test_table_alignment() {
        let left = TableAlignment::Left.to_markdown();
        let center = TableAlignment::Center.to_markdown();
        let right = TableAlignment::Right.to_markdown();

        assert_eq!(left, ":---");
        assert_eq!(center, ":---:");
        assert_eq!(right, "---:");
    }

    #[test]
    fn test_empty_table() {
        let html = "<table></table>";
        let tables = extract_tables_from_html(html).unwrap();
        assert_eq!(tables.len(), 0);
    }

    #[test]
    fn test_multiple_tables() {
        let html = r#"
            <table>
                <tr><th>A</th></tr>
                <tr><td>1</td></tr>
            </table>
            <p>Some text</p>
            <table>
                <tr><th>B</th></tr>
                <tr><td>2</td></tr>
            </table>
        "#;

        let tables = extract_tables_from_html(html).unwrap();
        assert_eq!(tables.len(), 2);
    }

    #[test]
    fn test_table_info_generation() {
        let html = r#"
            <table>
                <tr><th>Name</th><th>Age</th></tr>
                <tr><td>Alice</td><td>30</td></tr>
            </table>
        "#;

        let info = get_table_info(html).unwrap();
        assert!(info.contains("Total tables: 1"));
        assert!(info.contains("Columns: 2"));
        assert!(info.contains("Rows: 1"));
    }
}
