use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum ConversionError {
    #[error("Missing parameter: {0}")]
    MissingParameter(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid file type: {0}")]
    InvalidFileType(String),

    #[error("Conversion error: {0}")]
    ConversionFailed(String),
}
