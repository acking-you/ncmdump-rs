use base64::DecodeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NcmError {
    #[error("not a valid NCM file (bad magic)")]
    InvalidMagic,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("decryption failed: {0}")]
    Decrypt(String),
    #[error("base64 decode error: {0}")]
    Base64(#[from] DecodeError),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported audio format")]
    UnsupportedFormat,
    #[error("tagging error: {0}")]
    Tag(String),
}

pub type Result<T> = std::result::Result<T, NcmError>;
