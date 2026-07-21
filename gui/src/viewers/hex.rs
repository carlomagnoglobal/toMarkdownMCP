//! Hex dump file viewer implementation

use super::traits::{FileViewer, ViewerError, ViewerState};
use std::path::PathBuf;

/// Viewer for binary files displayed as hexadecimal
///
/// Displays file content as a hex dump with offset, hex bytes, and ASCII representation.
/// Each row displays 16 bytes for compact readability.
pub struct HexViewer {
    /// Path to the binary file
    path: PathBuf,
    /// Raw binary data
    bytes: Vec<u8>,
    /// Total file size in bytes
    total_size: u64,
}

impl HexViewer {
    /// Creates a new HexViewer from binary data
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file being viewed
    /// * `bytes` - Binary data to display
    /// * `total_size` - Total file size in bytes
    ///
    /// # Errors
    ///
    /// Returns `ViewerError` if the bytes vector is too large or other validation fails
    pub fn new_from_bytes(path: PathBuf, bytes: Vec<u8>, total_size: u64) -> Result<Self, ViewerError> {
        Ok(HexViewer {
            path,
            bytes,
            total_size,
        })
    }

    /// Formats a single line of hex output with 16 bytes
    ///
    /// # Arguments
    ///
    /// * `offset` - Current offset in the file
    /// * `chunk` - Slice of up to 16 bytes to format
    ///
    /// Returns a formatted HTML table row
    fn format_hex_line(offset: u64, chunk: &[u8]) -> String {
        // Format offset as 8 hex digits
        let offset_str = format!("{:08X}", offset);

        // Format hex bytes as space-separated pairs
        let hex_str = chunk
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");

        // Format ASCII representation (printable chars or dots)
        let ascii_str = chunk
            .iter()
            .map(|b| {
                if b.is_ascii_graphic() || *b == b' ' {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect::<String>();

        format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            offset_str, hex_str, ascii_str
        )
    }
}

impl FileViewer for HexViewer {
    fn render(&self) -> Result<String, ViewerError> {
        let mut html = String::from("<table>");

        // Process bytes in 16-byte chunks
        for (i, chunk) in self.bytes.chunks(16).enumerate() {
            let offset = (i as u64) * 16;
            let row = Self::format_hex_line(offset, chunk);
            html.push_str(&row);
        }

        html.push_str("</table>");
        Ok(html)
    }

    fn get_state(&self) -> ViewerState {
        ViewerState {
            file_type: "hex".to_string(),
            file_path: self.path.clone(),
            modified: false,
            file_size_bytes: self.total_size,
        }
    }

    fn file_type(&self) -> &str {
        "hex"
    }
}
