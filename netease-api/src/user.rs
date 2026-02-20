//! User profile API.
//!
//! Endpoint: `POST /weapi/nuser/account/get`
//!
//! Request: `{}` (empty object, authentication is via cookie).
//!
//! Response:
//! ```json
//! {
//!   "code": 200,
//!   "profile": {
//!     "userId": 413184081,
//!     "nickname": "用户名",
//!     "avatarUrl": "https://p1.music.126.net/..."
//!   }
//! }
//! ```
//!
//! Returns code 301 if the cookie is invalid or expired.

use crate::client::NeteaseClient;
use crate::error::{NeteaseError, Result};
use crate::types::UserProfile;
use serde_json::json;

impl NeteaseClient {
    /// Get the current logged-in user's profile.
    ///
    /// # Errors
    ///
    /// - [`NeteaseError::NotLoggedIn`] — no `MUSIC_U` cookie configured
    /// - [`NeteaseError::Api`] with code 301 — cookie expired
    pub fn user_info(&self) -> Result<UserProfile> {
        if !self.session().is_logged_in() {
            return Err(NeteaseError::NotLoggedIn);
        }
        let data = json!({});
        let resp = self.request("/nuser/account/get", &data)?;
        let p = &resp["profile"];
        Ok(UserProfile {
            id: p["userId"].as_u64().unwrap_or(0),
            nickname: p["nickname"].as_str().unwrap_or("").to_owned(),
            avatar_url: p["avatarUrl"].as_str().map(String::from),
        })
    }
}
