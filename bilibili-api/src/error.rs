//! Error types for the Bilibili API client.

use thiserror::Error;

/// Errors that can occur when interacting with the Bilibili API.
#[derive(Debug, Error)]
pub enum BilibiliError {
    /// HTTP transport error.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// The API returned a non-zero `code` in its JSON response.
    #[error("API error (code {code}): {message}")]
    Api { code: i64, message: String },

    /// No valid session cookies configured.
    #[error("not logged in")]
    NotLoggedIn,

    /// File I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parse error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// ffmpeg conversion failed.
    #[error("ffmpeg error: {0}")]
    Ffmpeg(String),

    /// QR login flow error.
    #[error("QR login: {0}")]
    QrLogin(String),

    /// Catch-all.
    #[error("{0}")]
    Other(String),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, BilibiliError>;
