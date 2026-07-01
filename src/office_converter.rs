use anyhow::{anyhow, Result};
use std::io::Read;
use std::path::Path;

/// Extensions handled by this module.
pub fn is_office_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "xlsx" | "xls" | "xlsm" | "ods" | "csv" | "pptx" | "ppt" | "odp"
    )
}

/// Convert a spreadsheet/presentation file to Markdown.
pub fn convert_office(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "xlsx" | "xls" | "xlsm" | "ods" => convert_spreadsheet(path),
        "csv" => convert_csv(path),
        "pptx" | "odp" => convert_pptx(path),
        "ppt" => Ok("> **Note:** Legacy binary `.ppt` is not supported. Please convert to \
                     `.pptx` and try again."
            .to_string()),
        other => Err(anyhow!("Unsupported office extension: {}", other)),
    }
}

// ----------------------------------------------------------------------------
// Spreadsheets (XLSX/XLS/ODS) via calamine
// ----------------------------------------------------------------------------

fn convert_spreadsheet(path: &Path) -> Result<String> {
    use calamine::{open_workbook_auto, Reader};

    let mut workbook =
        open_workbook_auto(path).map_err(|e| anyhow!("Failed to open spreadsheet: {}", e))?;

    let mut out = String::new();
    let sheet_names = workbook.sheet_names().to_owned();

    for name in sheet_names {
        let range = match workbook.worksheet_range(&name) {
            Ok(r) => r,
            Err(_) => continue,
        };
        out.push_str(&format!("## {}\n\n", name));
        if range.is_empty() {
            out.push_str("_(empty sheet)_\n\n");
            continue;
        }
        out.push_str(&range_to_markdown_table(&range));
        out.push('\n');
    }

    if out.trim().is_empty() {
        return Ok("_(no readable sheets found)_\n".to_string());
    }
    Ok(out)
}

/// Render a calamine range as a Markdown pipe table (first row = header).
fn range_to_markdown_table(range: &calamine::Range<calamine::Data>) -> String {
    let rows: Vec<Vec<String>> = range
        .rows()
        .map(|row| row.iter().map(cell_to_string).collect())
        .collect();

    rows_to_markdown_table(&rows)
}

fn cell_to_string(cell: &calamine::Data) -> String {
    use calamine::Data;
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => format_number(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(d) => format_number(d.as_f64()),
        other => other.to_string(),
    }
}

/// Format a float without a trailing ".0" for integer-valued numbers.
fn format_number(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        f.to_string()
    }
}

// ----------------------------------------------------------------------------
// CSV
// ----------------------------------------------------------------------------

fn convert_csv(path: &Path) -> Result<String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)
        .map_err(|e| anyhow!("Failed to open CSV: {}", e))?;

    let rows: Vec<Vec<String>> = reader
        .records()
        .filter_map(|r| r.ok())
        .map(|rec| rec.iter().map(|f| f.to_string()).collect())
        .collect();

    if rows.is_empty() {
        return Ok("_(empty CSV)_\n".to_string());
    }
    Ok(rows_to_markdown_table(&rows))
}

// ----------------------------------------------------------------------------
// Shared: rows -> Markdown table
// ----------------------------------------------------------------------------

/// Build a GitHub-flavored Markdown table from rows. The first row is treated
/// as the header. Cells are escaped for pipe characters and newlines.
fn rows_to_markdown_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }

    let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ").trim().to_string();
    let pad = |row: &[String]| -> Vec<String> {
        let mut v: Vec<String> = row.iter().map(|c| esc(c)).collect();
        while v.len() < cols {
            v.push(String::new());
        }
        v
    };

    let mut out = String::new();
    let header = pad(&rows[0]);
    out.push_str(&format!("| {} |\n", header.join(" | ")));
    out.push_str(&format!("| {} |\n", vec!["---"; cols].join(" | ")));
    for row in &rows[1..] {
        out.push_str(&format!("| {} |\n", pad(row).join(" | ")));
    }
    out
}

// ----------------------------------------------------------------------------
// Presentations (PPTX/ODP) via zip + quick-xml
// ----------------------------------------------------------------------------

fn convert_pptx(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow!("Failed to open presentation: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| anyhow!("Failed to read presentation: {}", e))?;

    // Collect slide XML entry names, sorted by slide number.
    let mut slide_names: Vec<String> = archive
        .file_names()
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .map(|n| n.to_string())
        .collect();
    slide_names.sort_by_key(|n| slide_number(n));

    // ODP stores everything in content.xml instead.
    if slide_names.is_empty() {
        if let Ok(mut entry) = archive.by_name("content.xml") {
            let mut xml = String::new();
            entry.read_to_string(&mut xml).ok();
            return Ok(odp_content_to_markdown(&xml));
        }
        return Ok("_(no slides found)_\n".to_string());
    }

    let mut out = String::new();
    for (idx, name) in slide_names.iter().enumerate() {
        let mut xml = String::new();
        if let Ok(mut entry) = archive.by_name(name) {
            entry.read_to_string(&mut xml).ok();
        }
        out.push_str(&format!("## Slide {}\n\n", idx + 1));
        let lines = pptx_slide_text(&xml);
        if lines.is_empty() {
            out.push_str("_(no text)_\n\n");
        } else {
            for line in lines {
                out.push_str(&format!("- {}\n", line));
            }
            out.push('\n');
        }
    }
    Ok(out)
}

/// Extract the numeric portion of a slide entry name for ordering.
fn slide_number(name: &str) -> u32 {
    name.trim_start_matches("ppt/slides/slide")
        .trim_end_matches(".xml")
        .parse()
        .unwrap_or(0)
}

/// Collect text from `<a:t>` runs in a PPTX slide, one entry per paragraph.
fn pptx_slide_text(xml: &str) -> Vec<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(false);

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_text = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().local_name().as_ref() {
                b"t" => in_text = true,
                b"p" => current.clear(),
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if in_text {
                    current.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => match e.name().local_name().as_ref() {
                b"t" => in_text = false,
                b"p" => {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        lines.push(trimmed.to_string());
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    lines
}

/// Best-effort ODP: split content.xml paragraphs, one bullet per non-empty one.
fn odp_content_to_markdown(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(false);

    let mut out = String::new();
    let mut current = String::new();
    let mut capture = false;
    let mut slide = 0;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().local_name().as_ref() {
                b"page" => {
                    slide += 1;
                    out.push_str(&format!("## Slide {}\n\n", slide));
                }
                b"p" | b"span" => {
                    capture = true;
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if capture {
                    current.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => {
                if e.name().local_name().as_ref() == b"p" {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        out.push_str(&format!("- {}\n", trimmed));
                    }
                    current.clear();
                    capture = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    if out.trim().is_empty() {
        "_(no text found)_\n".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rows_to_markdown_table() {
        let rows = vec![
            vec!["Name".to_string(), "Age".to_string()],
            vec!["Alice".to_string(), "30".to_string()],
            vec!["Bob".to_string(), "25".to_string()],
        ];
        let md = rows_to_markdown_table(&rows);
        assert!(md.contains("| Name | Age |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| Alice | 30 |"));
        assert!(md.contains("| Bob | 25 |"));
    }

    #[test]
    fn test_table_pads_short_rows_and_escapes_pipes() {
        let rows = vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["only one".to_string()],
            vec!["pi|pe".to_string(), "x".to_string()],
        ];
        let md = rows_to_markdown_table(&rows);
        assert!(md.contains("| only one |  |"), "got: {md}");
        assert!(md.contains("pi\\|pe"), "got: {md}");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(30.0), "30");
        assert_eq!(format_number(3.14), "3.14");
    }

    #[test]
    fn test_pptx_slide_text() {
        let xml = r#"<p:sld xmlns:a="x" xmlns:p="y"><p:cSld><p:spTree>
          <a:p><a:r><a:t>Title Here</a:t></a:r></a:p>
          <a:p><a:r><a:t>Bullet one</a:t></a:r></a:p>
        </p:spTree></p:cSld></p:sld>"#;
        let lines = pptx_slide_text(xml);
        assert_eq!(lines, vec!["Title Here", "Bullet one"]);
    }
}
