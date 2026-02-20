//! Netease Cloud Music API client library.
//!
//! Provides authenticated access to the Netease Cloud Music WEAPI, including
//! search, track detail/URL/lyric, playlist, and user profile endpoints.
//!
//! # Authentication
//!
//! All API calls require a valid `MUSIC_U` cookie obtained from a logged-in
//! browser session. The cookie is persisted to `~/.config/ncmdump/session.json`.
//!
//! ```no_run
//! use netease_api::auth::Session;
//! use netease_api::NeteaseClient;
//!
//! // Save cookie
//! let session = Session { music_u: Some("YOUR_MUSIC_U".into()) };
//! session.save().unwrap();
//!
//! // Create client (loads session from disk)
//! let client = NeteaseClient::new().unwrap();
//! ```
//!
//! # API endpoint mapping
//!
//! | Method                  | WEAPI endpoint                  | Description          |
//! |-------------------------|---------------------------------|----------------------|
//! | [`NeteaseClient::search`]         | `/cloudsearch/get/web`  | Search music         |
//! | [`NeteaseClient::track_detail`]   | `/song/detail`          | Track metadata       |
//! | [`NeteaseClient::track_url`]      | `/song/enhance/player/url` | Playback URL      |
//! | [`NeteaseClient::track_lyric`]    | `/song/lyric`           | LRC lyrics           |
//! | [`NeteaseClient::download_track`] | (uses `track_url`)      | Download audio file  |
//! | [`NeteaseClient::playlist_detail`]| `/v6/playlist/detail`   | Playlist with tracks |
//! | [`NeteaseClient::user_info`]      | `/nuser/account/get`    | Current user profile |
//!
//! # Encryption
//!
//! All requests use the WEAPI encryption scheme (double AES-128-CBC + RSA),
//! matching the Netease web client. See [`crypto`](crate::crypto) (internal).

pub mod auth;
pub mod client;
mod crypto;
pub mod error;
mod playlist;
mod search;
mod track;
pub mod types;
mod user;

pub use client::NeteaseClient;
pub use error::{NeteaseError, Result};
