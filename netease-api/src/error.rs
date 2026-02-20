//! Error types for the Netease Cloud Music API client.

use thiserror::Error;

/// Errors that can occur when interacting with the Netease API.
#[derive(Debug, Error)]
pub enum NeteaseError {
    /// HTTP transport error (connection refused, timeout, TLS failure, etc.).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// The API returned a non-200 `code` in its JSON response body.
    ///
    /// Common codes:
    /// - `301`  — not logged in / cookie expired
    /// - `403`  — access denied (VIP required or region-locked)
    /// - `-460` — cheating detected (request too frequent)
    #[error("API error (code {code}): {message}")]
    Api {
        /// Netease API status code (not HTTP status).
        code: i64,
        /// Human-readable error message from the API.
        message: String,
    },

    /// No `MUSIC_U` cookie is configured. Call `login` first.
    #[error("not logged in")]
    NotLoggedIn,

    /// File I/O error (session read/write, download write).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse JSON response from the API.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Catch-all for other errors (e.g. missing config directory).
    #[error("{0}")]
    Other(String),
}

/// Convenience alias for `Result<T, NeteaseError>`.
pub type Result<T> = std::result::Result<T, NeteaseError>;
