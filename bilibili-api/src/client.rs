//! HTTP client for Bilibili web API.
//!
//! Handles WBI-signed GET requests, session cookies, and wbi_keys caching.

use crate::auth::{BiliSession, QrCodeGenerate, QrPollStatus};
use crate::error::{BilibiliError, Result};
use crate::wbi;
use reqwest::blocking::Client;
use serde_json::Value;
use std::cell::RefCell;

const API_BASE: &str = "https://api.bilibili.com";
const PASSPORT_BASE: &str = "https://passport.bilibili.com";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const REFERER: &str = "https://www.bilibili.com/";
const ORIGIN: &str = "https://www.bilibili.com";

/// Cached WBI keys (img_key, sub_key).
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
            .timeout(std::time::Duration::from_secs(30))
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

        let mut req = self.http.get(format!("{API_BASE}/x/web-interface/nav"))
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
            url.rsplit('/').next().unwrap_or("")
                .rsplit_once('.').map_or("", |(name, _)| name)
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
        let mut req = self.http.get(&url)
            .query(&signed)
            .header("Referer", REFERER)
            .header("Origin", ORIGIN);
        if let Some(cookie) = self.session.cookie_header() {
            req = req.header("Cookie", &cookie);
        }

        let resp: Value = req.send()?.json()?;
        let code = resp["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            let msg = resp["message"].as_str().unwrap_or("unknown error").to_owned();
            return Err(BilibiliError::Api { code, message: msg });
        }
        Ok(resp)
    }

    /// Send a plain GET request (no WBI signing).
    pub fn get(&self, url: &str) -> Result<Value> {
        let mut req = self.http.get(url)
            .header("Referer", REFERER)
            .header("Origin", ORIGIN);
        if let Some(cookie) = self.session.cookie_header() {
            req = req.header("Cookie", &cookie);
        }
        let resp: Value = req.send()?.json()?;
        Ok(resp)
    }

    /// Download raw bytes from a URL with proper Bilibili headers.
    pub fn download_raw(&self, url: &str, dest: &std::path::Path) -> Result<u64> {
        let resp = self.http.get(url)
            .header("Referer", REFERER)
            .header("Origin", ORIGIN)
            .header("User-Agent", USER_AGENT)
            .send()?;
        let bytes = resp.bytes()?;
        std::fs::write(dest, &bytes)?;
        Ok(bytes.len() as u64)
    }

    // ── QR login ──

    /// Generate a QR code for login.
    pub fn qr_generate(&self) -> Result<QrCodeGenerate> {
        let url = format!("{PASSPORT_BASE}/x/passport-login/web/qrcode/generate");
        let resp: Value = self.http.get(&url)
            .header("Referer", REFERER)
            .send()?.json()?;

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
        let url = format!(
            "{PASSPORT_BASE}/x/passport-login/web/qrcode/poll?qrcode_key={qrcode_key}"
        );
        let resp = self.http.get(&url)
            .header("Referer", REFERER)
            .send()?;

        // Extract Set-Cookie headers before consuming body.
        let cookies: Vec<String> = resp.headers()
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
                let msg = json["data"]["message"].as_str().unwrap_or("unknown").to_owned();
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
