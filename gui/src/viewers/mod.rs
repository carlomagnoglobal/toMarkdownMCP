//! File viewer trait implementations and types

pub mod hex;
pub mod markdown;
pub mod traits;

// Re-export public types for convenient access
pub use hex::HexViewer;
pub use markdown::MarkdownViewer;
pub use traits::{FileViewer, ViewerError, ViewerState};
