//! Playlist API.
//!
//! Endpoint: `POST /weapi/v6/playlist/detail`
//!
//! Request: `{ "id": 123456, "n": 100000 }`
//!
//! The `n` parameter controls how many tracks to include in the response
//! (100000 = "all tracks"). Without it, the API returns only track IDs.
//!
//! Response:
//! ```json
//! {
//!   "code": 200,
//!   "playlist": {
//!     "id": 123456,
//!     "name": "歌单名",
//!     "description": "描述...",
//!     "coverImgUrl": "https://...",
//!     "trackCount": 50,
//!     "creator": { "userId": 789, "nickname": "用户名" },
//!     "tracks": [
//!       { "id": 1, "name": "歌名", "ar": [...], "al": {...}, "dt": 240000 },
//!       ...
//!     ]
//!   }
//! }
//! ```

use crate::client::NeteaseClient;
use crate::error::Result;
use crate::types::{Album, Artist, Playlist, Track, UserBrief};
use serde_json::{Value, json};

impl NeteaseClient {
    /// Get playlist detail including all tracks.
    ///
    /// Returns a [`Playlist`] with the `tracks` field populated.
    /// Does not require login for public playlists.
    pub fn playlist_detail(&self, id: u64) -> Result<Playlist> {
        let data = json!({ "id": id, "n": 100_000 });
        let resp = self.request("/v6/playlist/detail", &data)?;
        let p = &resp["playlist"];
        Ok(Playlist {
            id: p["id"].as_u64().unwrap_or(0),
            name: p["name"].as_str().unwrap_or("").to_owned(),
            description: p["description"].as_str().map(String::from),
            cover_url: p["coverImgUrl"].as_str().map(String::from),
            track_count: p["trackCount"].as_u64().unwrap_or(0),
            creator: parse_creator(&p["creator"]),
            tracks: p["tracks"]
                .as_array()
                .map(|arr| arr.iter().map(parse_track).collect()),
        })
    }
}

fn parse_creator(v: &Value) -> Option<UserBrief> {
    if v.is_null() {
        return None;
    }
    Some(UserBrief {
        id: v["userId"].as_u64().unwrap_or(0),
        name: v["nickname"].as_str().unwrap_or("").to_owned(),
    })
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
    Track {
        id: v["id"].as_u64().unwrap_or(0),
        name: v["name"].as_str().unwrap_or("").to_owned(),
        artists,
        album: Album {
            id: al["id"].as_u64().unwrap_or(0),
            name: al["name"].as_str().unwrap_or("").to_owned(),
            pic_url: al["picUrl"].as_str().map(String::from),
        },
        duration_ms: v["dt"]
            .as_u64()
            .or_else(|| v["duration"].as_u64())
            .unwrap_or(0),
    }
}
