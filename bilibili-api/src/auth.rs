//! Bilibili session management — QR login + cookie persistence.
//!
//! Session file: `~/.config/ncmdump/bilibili_session.json`
//!
//! ```json
//! {
//!   "sessdata": "...",
//!   "bili_jct": "...",
//!   "dede_user_id": "...",
//!   "buvid3": "...",
//!   "buvid4": ""
//! }
//! ```

use crate::error::{BilibiliError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Persistent Bilibili login session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BiliSession {
    #[serde(default)]
    pub sessdata: Option<String>,
    #[serde(default)]
    pub bili_jct: Option<String>,
    #[serde(default)]
    pub dede_user_id: Option<String>,
    #[serde(default)]
    pub buvid3: Option<String>,
    #[serde(default)]
    pub buvid4: Option<String>,
}

impl BiliSession {
    /// Load session from disk. Returns default if file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data)?)
    }

    /// Save session to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Delete session file.
    pub fn clear() -> Result<()> {
        let path = Self::path()?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Build the `Cookie` header value for API requests.
    pub fn cookie_header(&self) -> Option<String> {
        let sessdata = self.sessdata.as_deref()?;
        let mut parts = vec![format!("SESSDATA={sessdata}")];
        if let Some(jct) = &self.bili_jct {
            parts.push(format!("bili_jct={jct}"));
        }
        if let Some(uid) = &self.dede_user_id {
            parts.push(format!("DedeUserID={uid}"));
        }
        if let Some(b3) = &self.buvid3 {
            parts.push(format!("buvid3={b3}"));
        }
        if let Some(b4) = &self.buvid4 {
            parts.push(format!("buvid4={b4}"));
        }
        Some(parts.join("; "))
    }

    /// Check whether a SESSDATA cookie is present.
    pub fn is_logged_in(&self) -> bool {
        self.sessdata.as_ref().is_some_and(|s| !s.is_empty())
    }

    fn path() -> Result<PathBuf> {
        let config = dirs::config_dir()
            .ok_or_else(|| BilibiliError::Other("cannot determine config directory".into()))?;
        Ok(config.join("ncmdump").join("bilibili_session.json"))
    }
}

/// QR code login response from generate endpoint.
#[derive(Debug, Deserialize)]
pub struct QrCodeGenerate {
    pub url: String,
    pub qrcode_key: String,
}

/// QR code poll status.
#[derive(Debug)]
pub enum QrPollStatus {
    /// Waiting for scan.
    Waiting,
    /// Scanned, waiting for confirm.
    Scanned,
    /// Login successful — session extracted.
    Success(BiliSession),
    /// Expired — need to regenerate.
    Expired,
}
