//! HTTP client for Netease Cloud Music WEAPI.
//!
//! All requests go through the WEAPI encryption scheme:
//!
//! 1. Serialize parameters as JSON
//! 2. Double AES-128-CBC encrypt → `params` (base64)
//! 3. RSA encrypt the random AES key → `encSecKey` (hex)
//! 4. POST to `https://music.163.com/weapi{endpoint}` with URL-encoded body
//!
//! The server responds with JSON containing a `code` field (200 = success).
//!
//! # Response format
//!
//! All API responses share this envelope:
//!
//! ```json
//! {
//!   "code": 200,
//!   ...endpoint-specific fields...
//! }
//! ```
//!
//! Non-200 codes are mapped to [`NeteaseError::Api`](crate::NeteaseError::Api).

use crate::auth::Session;
use crate::crypto::weapi_encrypt;
use crate::error::{NeteaseError, Result};
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const BASE_URL: &str = "https://music.163.com";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Blocking HTTP client for the Netease Cloud Music API.
///
/// Holds a [`reqwest::blocking::Client`] and a [`Session`] (cookie store).
/// API methods are implemented in separate modules (`search`, `track`,
/// `playlist`, `user`) as `impl NeteaseClient` blocks.
pub struct NeteaseClient {
    http: Client,
    session: Session,
}

impl NeteaseClient {
    /// Create a new client, loading the session from
    /// `~/.config/ncmdump/session.json`.
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let session = Session::load()?;
        Ok(Self { http, session })
    }

    /// Create a client with an explicit [`Session`] (useful for testing
    /// or when the cookie is provided programmatically).
    pub fn with_session(session: Session) -> Result<Self> {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { http, session })
    }

    /// Return a reference to the current session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Send a WEAPI-encrypted POST request to the given endpoint.
    ///
    /// `endpoint` is the path after `/weapi`, e.g. `/song/detail`.
    /// `data` is the JSON object to encrypt and send as the request body.
    ///
    /// Returns the full JSON response on success (code 200).
    /// Returns [`NeteaseError::Api`] if the response `code` is not 200.
    pub fn request(&self, endpoint: &str, data: &Value) -> Result<Value> {
        let payload = weapi_encrypt(&data.to_string());
        let url = format!("{BASE_URL}/weapi{endpoint}");

        let mut req = self
            .http
            .post(&url)
            .header("Referer", "https://music.163.com")
            .header("Content-Type", "application/x-www-form-urlencoded");

        if let Some(cookie) = self.session.cookie_header() {
            req = req.header("Cookie", cookie);
        }

        let body = format!(
            "params={}&encSecKey={}",
            urlencoding::encode(&payload.params),
            payload.enc_sec_key,
        );

        let resp = req.body(body).send()?;
        let json: Value = resp.json()?;

        if let Some(code) = json.get("code").and_then(Value::as_i64) {
            if code != 200 {
                let msg = json
                    .get("message")
                    .or_else(|| json.get("msg"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
                    .to_owned();
                return Err(NeteaseError::Api { code, message: msg });
            }
        }

        Ok(json)
    }

    /// Download a file from `url` and write it to `dest`.
    ///
    /// Used internally by [`download_track`](Self::download_track) but can
    /// also be called directly with any URL (e.g. album cover images).
    ///
    /// Returns the number of bytes written.
    pub fn download(&self, url: &str, dest: &Path) -> Result<u64> {
        let resp = self
            .http
            .get(url)
            .header("Referer", "https://music.163.com/")
            .send()?;

        let mut file = File::create(dest)?;
        let bytes = resp.bytes()?;
        file.write_all(&bytes)?;
        Ok(bytes.len() as u64)
    }
}
