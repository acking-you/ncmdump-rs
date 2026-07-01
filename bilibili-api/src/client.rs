//! HTTP client for Bilibili web API.
//!
//! Handles WBI-signed GET requests, session cookies, and `wbi_keys` caching.

use crate::auth::{BiliSession, QrCodeGenerate, QrPollStatus};
use crate::error::{BilibiliError, Result};
use crate::wbi;
use download_core::{DownloadProgressEvent, DownloadProgressPhase, DownloadProgressReporter};
use reqwest::blocking::Client;
use serde_json::Value;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Instant;

const API_BASE: &str = "https://api.bilibili.com";
const PASSPORT_BASE: &str = "https://passport.bilibili.com";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const REFERER: &str = "https://www.bilibili.com/";
const ORIGIN: &str = "https://www.bilibili.com";

/// Cached WBI keys (`img_key`, `sub_key`).
type WbiKeys = (String, String);

/// Blocking HTTP client for the Bilibili API.
pub struct BilibiliClient {
    http: Client,
    session: BiliSession,
    /// Cached WBI keys, fetched lazily from /x/web-interface/nav.
    wbi_keys: RefCell<Option<WbiKeys>>,
}

impl BilibiliClient {
    /// Create a new client, loading session from disk.
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()?;
        let session = BiliSession::load()?;
        Ok(Self {
            http,
            session,
            wbi_keys: RefCell::new(None),
        })
    }

    pub fn session(&self) -> &BiliSession {
        &self.session
    }

    /// Fetch and cache WBI keys from the nav API.
    fn ensure_wbi_keys(&self) -> Result<WbiKeys> {
        if let Some(keys) = self.wbi_keys.borrow().as_ref() {
            return Ok(keys.clone());
        }

        let mut req = self
            .http
            .get(format!("{API_BASE}/x/web-interface/nav"))
            .header("Referer", REFERER)
            .header("Origin", ORIGIN);
        if let Some(cookie) = self.session.cookie_header() {
            req = req.header("Cookie", &cookie);
        }

        let resp: Value = req.send()?.json()?;
        let data = &resp["data"]["wbi_img"];

        let img_url = data["img_url"].as_str().unwrap_or("");
        let sub_url = data["sub_url"].as_str().unwrap_or("");

        // Extract key from URL: last path segment without extension.
        let extract_key = |url: &str| -> String {
            url.rsplit('/')
                .next()
                .unwrap_or("")
                .rsplit_once('.')
                .map_or("", |(name, _)| name)
                .to_owned()
        };

        let keys = (extract_key(img_url), extract_key(sub_url));
        *self.wbi_keys.borrow_mut() = Some(keys.clone());
        Ok(keys)
    }

    /// Send a WBI-signed GET request.
    pub fn wbi_get(&self, path: &str, params: &[(String, String)]) -> Result<Value> {
        let (img_key, sub_key) = self.ensure_wbi_keys()?;
        let signed = wbi::sign_params(params, &img_key, &sub_key);

        let url = format!("{API_BASE}{path}");
        let mut req = self
            .http
            .get(&url)
            .query(&signed)
            .header("Referer", REFERER)
            .header("Origin", ORIGIN);
        if let Some(cookie) = self.session.cookie_header() {
            req = req.header("Cookie", &cookie);
        }

        let resp: Value = req.send()?.json()?;
        let code = resp["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            let msg = resp["message"]
                .as_str()
                .unwrap_or("unknown error")
                .to_owned();
            return Err(BilibiliError::Api { code, message: msg });
        }
        Ok(resp)
    }

    /// Send a plain GET request (no WBI signing).
    pub fn get(&self, url: &str) -> Result<Value> {
        let mut req = self
            .http
            .get(url)
            .header("Referer", REFERER)
            .header("Origin", ORIGIN);
        if let Some(cookie) = self.session.cookie_header() {
            req = req.header("Cookie", &cookie);
        }
        let resp: Value = req.send()?.json()?;
        Ok(resp)
    }

    /// Download raw bytes from a URL with proper Bilibili headers.
    ///
    /// Streams to disk with stall detection: aborts if transfer speed stays
    /// below 10 KB/s for 30 consecutive seconds.
    pub fn download_raw(&self, url: &str, dest: &std::path::Path) -> Result<u64> {
        self.download_raw_with_progress(
            url,
            dest,
            "download".to_string(),
            Arc::new(download_core::NoopProgressReporter),
            "bilibili".to_string(),
            "Downloading audio stream".to_string(),
        )
    }

    pub(crate) fn download_raw_with_progress(
        &self,
        url: &str,
        dest: &std::path::Path,
        job_id: String,
        reporter: Arc<dyn DownloadProgressReporter>,
        source: String,
        message: String,
    ) -> Result<u64> {
        let resp = self
            .http
            .get(url)
            .header("Referer", REFERER)
            .header("Origin", ORIGIN)
            .header("User-Agent", USER_AGENT)
            .send()?;
        let total_bytes = resp.content_length();

        let mut file = std::fs::File::create(dest)?;
        let mut reader = resp;
        const BUF_SIZE: usize = 32 * 1024;
        const STALL_THRESHOLD_BYTES: u64 = 10 * 1024; // 10 KB/s
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

    // ── QR login ──

    /// Generate a QR code for login.
    pub fn qr_generate(&self) -> Result<QrCodeGenerate> {
        let url = format!("{PASSPORT_BASE}/x/passport-login/web/qrcode/generate");
        let resp: Value = self
            .http
            .get(&url)
            .header("Referer", REFERER)
            .send()?
            .json()?;

        let code = resp["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            return Err(BilibiliError::QrLogin("failed to generate QR code".into()));
        }

        let data = &resp["data"];
        Ok(QrCodeGenerate {
            url: data["url"].as_str().unwrap_or("").to_owned(),
            qrcode_key: data["qrcode_key"].as_str().unwrap_or("").to_owned(),
        })
    }

    /// Poll QR code login status.
    pub fn qr_poll(&self, qrcode_key: &str) -> Result<QrPollStatus> {
        let url =
            format!("{PASSPORT_BASE}/x/passport-login/web/qrcode/poll?qrcode_key={qrcode_key}");
        let resp = self.http.get(&url).header("Referer", REFERER).send()?;

        // Extract Set-Cookie headers before consuming body.
        let cookies: Vec<String> = resp
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok().map(String::from))
            .collect();

        let json: Value = resp.json()?;
        let status_code = json["data"]["code"].as_i64().unwrap_or(-1);

        match status_code {
            0 => {
                // Success — extract session from cookies.
                let session = Self::extract_session_from_cookies(&cookies);
                Ok(QrPollStatus::Success(session))
            }
            86038 => Ok(QrPollStatus::Expired),
            86090 => Ok(QrPollStatus::Scanned),
            86101 => Ok(QrPollStatus::Waiting),
            _ => {
                let msg = json["data"]["message"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_owned();
                Err(BilibiliError::QrLogin(msg))
            }
        }
    }

    fn extract_session_from_cookies(cookies: &[String]) -> BiliSession {
        let mut session = BiliSession::default();
        for cookie in cookies {
            let kv = cookie.split(';').next().unwrap_or("");
            if let Some((key, val)) = kv.split_once('=') {
                match key.trim() {
                    "SESSDATA" => session.sessdata = Some(val.to_owned()),
                    "bili_jct" => session.bili_jct = Some(val.to_owned()),
                    "DedeUserID" => session.dede_user_id = Some(val.to_owned()),
                    "buvid3" => session.buvid3 = Some(val.to_owned()),
                    "buvid4" => session.buvid4 = Some(val.to_owned()),
                    _ => {}
                }
            }
        }
        session
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
