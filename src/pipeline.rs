//! Format-agnostic conversion entry points shared by the MCP server handlers
//! and the GUI: route any supported file to the right converter and apply the
//! large-file guardrails.

use std::path::Path;

use crate::error::ConversionError;
use crate::{document_converter, feed_email_converter, markup_converter, office_converter};

/// Above this size, structured formats are refused (their parsers hold the
/// whole transformed document in memory, typically several times the input)
/// and plain text/code switches to a single-pass buffered read. The
/// `max_bytes` tool parameter overrides it.
pub const LARGE_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// True when the extension belongs to a structured format whose converter
/// must load and transform the entire file (HTML family, markup, binary docs).
pub fn is_structured_ext(ext: Option<&str>) -> bool {
    let Some(ext) = ext else { return false };
    matches!(ext, "html" | "htm" | "mhtml" | "webarchive")
        || markup_converter::is_markup_extension(ext)
        || document_converter::is_document_extension(ext)
        || office_converter::is_office_extension(ext)
        || feed_email_converter::is_feed_email_extension(ext)
}

pub fn large_file_error(path: &str, size: u64, limit: u64) -> Box<dyn std::error::Error> {
    Box::new(ConversionError::ConversionFailed(format!(
        "{} is {:.1} MB, above the {:.0} MB limit for structured conversion. \
         Use get_file_summary for an overview, chunk_markdown/extract_chunks_for_rag \
         for piecewise processing, or pass max_bytes to raise the limit.",
        path,
        size as f64 / (1024.0 * 1024.0),
        limit as f64 / (1024.0 * 1024.0),
    )))
}

/// If the extension is a binary/office document format, convert it to Markdown.
/// Returns None for formats handled by the normal text pipeline.
pub fn try_convert_binary_document(path: &Path) -> Option<Result<String, Box<dyn std::error::Error>>> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    let map_err = |e: anyhow::Error| {
        Box::new(ConversionError::ConversionFailed(e.to_string())) as Box<dyn std::error::Error>
    };
    if document_converter::is_document_extension(ext) {
        Some(document_converter::convert_document(path).map_err(map_err))
    } else if office_converter::is_office_extension(ext) {
        Some(office_converter::convert_office(path).map_err(map_err))
    } else if feed_email_converter::is_feed_email_extension(ext) {
        Some(feed_email_converter::convert_feed_email(path).map_err(map_err))
    } else {
        None
    }
}

/// Convert any supported file to Markdown/plain text for RAG/analysis and
/// viewing. Code/text files are returned as-is (no code fence).
pub fn convert_any_to_markdown(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size > LARGE_FILE_BYTES && is_structured_ext(path.extension().and_then(|e| e.to_str())) {
        return Err(large_file_error(&path.display().to_string(), size, LARGE_FILE_BYTES));
    }
    if let Some(conv) = try_convert_binary_document(path) {
        return conv;
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| Box::new(ConversionError::IoError(e.to_string())) as Box<dyn std::error::Error>)?;
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if markup_converter::is_markup_extension(ext) {
            return Ok(markup_converter::convert_markup(ext, &content));
        }
    }
    Ok(content)
}
