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
use download_core::{DownloadProgressEvent, DownloadProgressPhase, DownloadProgressReporter};
use reqwest::Proxy;
use reqwest::blocking::{Client, ClientBuilder};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const BASE_URL: &str = "https://music.163.com";
const NETEASE_PROXY_URL_ENV: &str = "NETEASE_RELAY_URL";
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
        let http = build_http_client()?;
        let session = Session::load()?;
        Ok(Self { http, session })
    }

    /// Create a client with an explicit [`Session`] (useful for testing
    /// or when the cookie is provided programmatically).
    pub fn with_session(session: Session) -> Result<Self> {
        let http = build_http_client()?;
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
    /// Streams to disk with stall detection: aborts if transfer speed stays
    /// below 10 KB/s for 30 consecutive seconds.
    ///
    /// Used internally by [`download_track`](Self::download_track) but can
    /// also be called directly with any URL (e.g. album cover images).
    ///
    /// Returns the number of bytes written.
    pub fn download(&self, url: &str, dest: &Path) -> Result<u64> {
        self.download_with_progress(
            url,
            dest,
            "download".to_string(),
            Arc::new(download_core::NoopProgressReporter),
            "netease".to_string(),
            "Downloading audio stream".to_string(),
        )
    }

    pub(crate) fn download_with_progress(
        &self,
        url: &str,
        dest: &Path,
        job_id: String,
        reporter: Arc<dyn DownloadProgressReporter>,
        source: String,
        message: String,
    ) -> Result<u64> {
        let resp = self
            .http
            .get(url)
            .header("Referer", "https://music.163.com/")
            .send()?;
        let total_bytes = resp.content_length();

        let mut file = std::fs::File::create(dest)?;
        let mut reader = resp;
        const BUF_SIZE: usize = 32 * 1024;
        const STALL_THRESHOLD_BYTES: u64 = 10 * 1024;
        const STALL_WINDOW_SECS: u64 = 30;
        const REPORT_EVERY_BYTES: u64 = 512 * 1024;

        let mut buf = [0u8; BUF_SIZE];
        let mut total: u64 = 0;
        let mut window_start = Instant::now();
        let mut window_bytes: u64 = 0;
        let mut last_reported = 0;

        reporter.emit(DownloadProgressEvent {
            job_id: job_id.clone(),
            source: source.clone(),
            phase: DownloadProgressPhase::Downloading,
            percent: total_bytes.map(|_| 0),
            message: message.clone(),
            detail: total_bytes.map(|size| format!("0 / {size} bytes")),
        });

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    file.write_all(&buf[..n])?;
                    total += n as u64;
                    window_bytes += n as u64;

                    let elapsed = window_start.elapsed().as_secs();
                    if elapsed >= STALL_WINDOW_SECS {
                        let speed = window_bytes / elapsed.max(1);
                        if speed < STALL_THRESHOLD_BYTES {
                            let detail = format!(
                                "download stalled at {total} bytes ({speed} B/s over {elapsed}s)"
                            );
                            reporter.emit(DownloadProgressEvent {
                                job_id: job_id.clone(),
                                source: source.clone(),
                                phase: DownloadProgressPhase::Failed,
                                percent: progress_percent(total, total_bytes),
                                message: "Download stalled".to_string(),
                                detail: Some(detail.clone()),
                            });
                            return Err(
                                std::io::Error::new(std::io::ErrorKind::TimedOut, detail).into()
                            );
                        }
                        window_start = Instant::now();
                        window_bytes = 0;
                    }

                    if total.saturating_sub(last_reported) >= REPORT_EVERY_BYTES {
                        last_reported = total;
                        reporter.emit(DownloadProgressEvent {
                            job_id: job_id.clone(),
                            source: source.clone(),
                            phase: DownloadProgressPhase::Downloading,
                            percent: progress_percent(total, total_bytes),
                            message: message.clone(),
                            detail: download_detail(total, total_bytes),
                        });
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    reporter.emit(DownloadProgressEvent {
                        job_id: job_id.clone(),
                        source: source.clone(),
                        phase: DownloadProgressPhase::Failed,
                        percent: progress_percent(total, total_bytes),
                        message: "Download failed".to_string(),
                        detail: Some(e.to_string()),
                    });
                    return Err(e.into());
                }
            }
        }

        file.flush()?;
        reporter.emit(DownloadProgressEvent {
            job_id,
            source,
            phase: DownloadProgressPhase::Downloading,
            percent: progress_percent(total, total_bytes).or(Some(100)),
            message,
            detail: download_detail(total, total_bytes),
        });
        Ok(total)
    }
}

fn progress_percent(downloaded: u64, total: Option<u64>) -> Option<u8> {
    total.and_then(|total| {
        if total == 0 {
            None
        } else {
            Some((downloaded.saturating_mul(100) / total).min(100) as u8)
        }
    })
}

fn download_detail(downloaded: u64, total: Option<u64>) -> Option<String> {
    total
        .map(|total| format!("{downloaded} / {total} bytes"))
        .or_else(|| Some(format!("{downloaded} bytes")))
}

fn build_http_client() -> Result<Client> {
    let mut builder = ClientBuilder::new()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(30));

    if let Some(proxy_url) = netease_proxy_url() {
        builder = builder.proxy(Proxy::all(&proxy_url)?);
    }

    Ok(builder.build()?)
}

fn netease_proxy_url() -> Option<String> {
    std::env::var(NETEASE_PROXY_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::netease_proxy_url;

    const NETEASE_PROXY_URL_ENV: &str = "NETEASE_RELAY_URL";

    #[test]
    fn netease_proxy_url_ignores_missing_or_blank_env() {
        let original = std::env::var_os(NETEASE_PROXY_URL_ENV);

        unsafe {
            std::env::remove_var(NETEASE_PROXY_URL_ENV);
        }
        assert_eq!(netease_proxy_url(), None);

        unsafe {
            std::env::set_var(NETEASE_PROXY_URL_ENV, "   ");
        }
        assert_eq!(netease_proxy_url(), None);

        restore_env(original);
    }

    #[test]
    fn netease_proxy_url_reads_trimmed_env() {
        let original = std::env::var_os(NETEASE_PROXY_URL_ENV);

        unsafe {
            std::env::set_var(NETEASE_PROXY_URL_ENV, "  http://relay.example.com:7890  ");
        }
        assert_eq!(
            netease_proxy_url().as_deref(),
            Some("http://relay.example.com:7890")
        );

        restore_env(original);
    }

    fn restore_env(value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => unsafe { std::env::set_var(NETEASE_PROXY_URL_ENV, value) },
            None => unsafe { std::env::remove_var(NETEASE_PROXY_URL_ENV) },
        }
    }
}
