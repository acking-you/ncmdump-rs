//! Bilibili video/audio API client library.
//!
//! Provides authenticated access to Bilibili's web API for searching videos,
//! fetching DASH audio streams, and downloading audio with ffmpeg conversion.
//!
//! # Authentication
//!
//! Bilibili requires a logged-in session for high-quality audio streams.
//! Use QR code login via [`BilibiliClient::qr_login`] or manually provide
//! session cookies. The session is persisted to `~/.config/ncmdump/bilibili_session.json`.
//!
//! # Audio download pipeline
//!
//! 1. `video_detail(bvid)` → get cid
//! 2. `dash_audio(bvid, cid, quality)` → DASH audio stream URLs
//! 3. `download_audio(bvid, output, format)` → download + ffmpeg convert

pub mod auth;
pub mod client;
pub mod download;
pub mod error;
pub mod search;
pub mod types;
pub mod video;
pub mod wbi;

pub use client::BilibiliClient;
pub use error::{BilibiliError, Result};
