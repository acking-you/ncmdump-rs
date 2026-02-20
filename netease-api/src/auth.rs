//! Session management — persists `MUSIC_U` cookie to disk.
//!
//! The session file is stored at `~/.config/ncmdump/session.json` and contains:
//!
//! ```json
//! { "MUSIC_U": "00AABBCC..." }
//! ```
//!
//! The `MUSIC_U` cookie is the authentication token issued by Netease after
//! login. It can be obtained from browser developer tools → Application → Cookies
//! on `music.163.com`. Typical lifetime is several months.

use crate::error::{NeteaseError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Persistent login session backed by a JSON file on disk.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Session {
    /// The `MUSIC_U` authentication cookie value.
    #[serde(rename = "MUSIC_U")]
    pub music_u: Option<String>,
}

impl Session {
    /// Load session from `~/.config/ncmdump/session.json`.
    ///
    /// Returns a default (empty) session if the file does not exist.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data)?)
    }

    /// Save session to disk, creating parent directories if needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Delete the session file from disk.
    pub fn clear() -> Result<()> {
        let path = Self::path()?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Build the `Cookie` HTTP header value for API requests.
    ///
    /// Returns `None` if no `MUSIC_U` is set. The header includes fixed
    /// fields expected by the Netease server: `os=pc`, `__remember_me=true`.
    pub fn cookie_header(&self) -> Option<String> {
        let music_u = self.music_u.as_deref()?;
        Some(format!("os=pc; __remember_me=true; MUSIC_U={music_u}"))
    }

    /// Check whether a `MUSIC_U` cookie is present (does not validate it).
    pub fn is_logged_in(&self) -> bool {
        self.music_u.as_ref().is_some_and(|u| !u.is_empty())
    }

    fn path() -> Result<PathBuf> {
        let config = dirs::config_dir()
            .ok_or_else(|| NeteaseError::Other("cannot determine config directory".into()))?;
        Ok(config.join("ncmdump").join("session.json"))
    }
}
