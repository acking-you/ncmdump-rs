//! Track detail, URL, lyric, and download APIs.
//!
//! # Endpoints
//!
//! ## `track_detail` — `POST /weapi/song/detail`
//!
//! Request: `{ "c": "[{\"id\":123}]", "ids": "[123]" }`
//!
//! Response:
//! ```json
//! {
//!   "code": 200,
//!   "songs": [{
//!     "id": 123, "name": "歌名",
//!     "ar": [{ "id": 1, "name": "歌手" }],
//!     "al": { "id": 2, "name": "专辑", "picUrl": "https://..." },
//!     "dt": 240000
//!   }]
//! }
//! ```
//!
//! ## `track_url` — `POST /weapi/song/enhance/player/url`
//!
//! Request: `{ "ids": "[123]", "br": 320000 }`
//!
//! Response:
//! ```json
//! {
//!   "code": 200,
//!   "data": [{
//!     "id": 123,
//!     "url": "https://m701.music.126.net/...",  // null if unavailable
//!     "br": 320000,
//!     "size": 12345678,
//!     "type": "mp3"
//!   }]
//! }
//! ```
//!
//! `url` is `null` when the track requires VIP/purchase or is region-locked.
//!
//! ## `track_lyric` — `POST /weapi/song/lyric`
//!
//! Request: `{ "id": 123, "lv": -1, "tv": -1 }`
//!
//! Response:
//! ```json
//! {
//!   "code": 200,
//!   "lrc":    { "lyric": "[00:00.00]歌词..." },
//!   "tlyric": { "lyric": "[00:00.00]翻译..." }
//! }
//! ```
//!
//! `lrc`/`tlyric` may be absent or have empty `lyric` for instrumental tracks.

use crate::client::NeteaseClient;
use crate::error::{NeteaseError, Result};
use crate::types::{Album, Artist, Lyric, Quality, Track};
use serde_json::{Value, json};
use std::path::Path;

impl NeteaseClient {
    /// Get track metadata by ID.
    ///
    /// Returns a [`Track`] with artist, album, and duration info.
    /// Does not require login for public tracks.
    pub fn track_detail(&self, id: u64) -> Result<Track> {
        let data = json!({
            "c": format!("[{{\"id\":{}}}]", id),
            "ids": format!("[{}]", id),
        });
        let resp = self.request("/song/detail", &data)?;
        let songs = resp["songs"]
            .as_array()
            .ok_or_else(|| NeteaseError::Other("missing songs".into()))?;
        let song = songs
            .first()
            .ok_or_else(|| NeteaseError::Other(format!("track not found: {id}")))?;
        Ok(parse_track(song))
    }

    /// Get a direct playback URL for a track at the requested quality.
    ///
    /// The returned URL is a temporary CDN link (typically valid for ~20 minutes)
    /// pointing to an MP3 or FLAC file. The server may downgrade quality if the
    /// user's VIP tier doesn't support the requested bitrate.
    ///
    /// # Errors
    ///
    /// Returns [`NeteaseError::Other`] if the track is unavailable (VIP-only,
    /// region-locked, or taken down — the API returns `url: null`).
    pub fn track_url(&self, id: u64, quality: Quality) -> Result<String> {
        let data = json!({
            "ids": format!("[{}]", id),
            "br": quality.bitrate(),
        });
        let resp = self.request("/song/enhance/player/url", &data)?;
        let url = resp["data"][0]["url"]
            .as_str()
            .ok_or_else(|| {
                NeteaseError::Other(
                    "track unavailable (no copyright or VIP required)".into(),
                )
            })?
            .to_owned();
        Ok(url)
    }

    /// Get lyrics for a track.
    ///
    /// Returns a [`Lyric`] with optional original (`lrc`) and translated
    /// (`tlyric`) lyrics in LRC timestamp format. Both fields are `None`
    /// for instrumental tracks or tracks without uploaded lyrics.
    pub fn track_lyric(&self, id: u64) -> Result<Lyric> {
        let data = json!({ "id": id, "lv": -1, "tv": -1 });
        let resp = self.request("/song/lyric", &data)?;
        Ok(Lyric {
            lrc: resp["lrc"]["lyric"].as_str().map(String::from),
            tlyric: resp["tlyric"]["lyric"].as_str().map(String::from),
        })
    }

    /// Download a track to a local file.
    ///
    /// Combines [`track_url`](Self::track_url) + [`download`](Self::download).
    /// Returns the number of bytes written to `dest`.
    pub fn download_track(
        &self,
        id: u64,
        quality: Quality,
        dest: &Path,
    ) -> Result<u64> {
        let url = self.track_url(id, quality)?;
        self.download(&url, dest)
    }
}

fn parse_track(v: &Value) -> Track {
    let artists = v["ar"]
        .as_array()
        .or_else(|| v["artists"].as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| Artist {
                    id: a["id"].as_u64().unwrap_or(0),
                    name: a["name"].as_str().unwrap_or("").to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();

    let al = if v["al"].is_null() { &v["album"] } else { &v["al"] };
    let album = Album {
        id: al["id"].as_u64().unwrap_or(0),
        name: al["name"].as_str().unwrap_or("").to_owned(),
        pic_url: al["picUrl"].as_str().map(String::from),
    };

    Track {
        id: v["id"].as_u64().unwrap_or(0),
        name: v["name"].as_str().unwrap_or("").to_owned(),
        artists,
        album,
        duration_ms: v["dt"]
            .as_u64()
            .or_else(|| v["duration"].as_u64())
            .unwrap_or(0),
    }
}
