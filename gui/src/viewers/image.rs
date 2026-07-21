//! Image file viewer implementation with metadata display

use super::traits::{FileViewer, ViewerError, ViewerState};
use std::path::PathBuf;

/// Viewer for image files
///
/// Displays images with metadata including format, dimensions, and file size.
/// Supports multiple image formats: png, jpeg, gif, svg, webp, avif, tiff.
pub struct ImageViewer {
    /// Path to the image file
    path: PathBuf,
    /// Image format (png, jpeg, gif, svg, webp, avif, tiff)
    format: String,
    /// Image width in pixels
    width: u32,
    /// Image height in pixels
    height: u32,
    /// File size in bytes
    file_size: u64,
}

impl ImageViewer {
    /// Creates a new ImageViewer
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the image file
    /// * `format` - Image format identifier (e.g., "png", "jpeg")
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    ///
    /// # Errors
    ///
    /// Returns `ViewerError` if parameters are invalid
    pub fn new(path: PathBuf, format: String, width: u32, height: u32) -> Result<Self, ViewerError> {
        // Validate parameters
        if format.is_empty() {
            return Err(ViewerError::ParseError("Format cannot be empty".to_string()));
        }

        // Try to get file size from metadata
        let file_size = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(ImageViewer {
            path,
            format,
            width,
            height,
            file_size,
        })
    }

    /// Creates a new ImageViewer with explicit file size
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the image file
    /// * `format` - Image format identifier (e.g., "png", "jpeg")
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `file_size` - File size in bytes
    ///
    /// # Errors
    ///
    /// Returns `ViewerError` if parameters are invalid
    pub fn new_with_size(
        path: PathBuf,
        format: String,
        width: u32,
        height: u32,
        file_size: u64,
    ) -> Result<Self, ViewerError> {
        // Validate parameters
        if format.is_empty() {
            return Err(ViewerError::ParseError("Format cannot be empty".to_string()));
        }

        Ok(ImageViewer {
            path,
            format,
            width,
            height,
            file_size,
        })
    }
}

impl FileViewer for ImageViewer {
    fn render(&self) -> Result<String, ViewerError> {
        // Properly escape the file path for URL
        let path_str = self.path.display().to_string();
        let file_url = if cfg!(target_os = "windows") {
            format!("file:///{}", path_str.replace('\\', "/"))
        } else {
            format!("file://{}", path_str)
        };

        // Format file size nicely
        let file_size_display = if self.file_size > 1024 * 1024 {
            format!("{:.2} MB", self.file_size as f64 / (1024.0 * 1024.0))
        } else if self.file_size > 1024 {
            format!("{:.2} KB", self.file_size as f64 / 1024.0)
        } else {
            format!("{} bytes", self.file_size)
        };

        // Build HTML with centered image and metadata
        let html = format!(
            r#"<div style="display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 20px; min-height: 100vh; background: var(--bg);">
    <img src="{}" style="max-width: 90%; max-height: 70vh; object-fit: contain; border-radius: 4px; box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1); margin-bottom: 20px;" alt="Image preview" />
    <div style="padding: 12px 16px; background-color: var(--code-bg); border: 1px solid var(--border); border-radius: 4px; text-align: center; font-family: monospace; font-size: 12px; color: var(--fg);">
        <div style="margin: 4px 0;"><strong>Format:</strong> {}</div>
        <div style="margin: 4px 0;"><strong>Dimensions:</strong> {} × {} px</div>
        <div style="margin: 4px 0;"><strong>File Size:</strong> {}</div>
    </div>
</div>"#,
            file_url,
            self.format.to_uppercase(),
            self.width,
            self.height,
            file_size_display
        );
        Ok(html)
    }

    fn get_state(&self) -> ViewerState {
        ViewerState {
            file_type: "image".to_string(),
            file_path: self.path.clone(),
            modified: false,
            file_size_bytes: self.file_size,
        }
    }

    fn file_type(&self) -> &str {
        "image"
    }
}
