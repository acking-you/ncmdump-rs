//! Search API.
//!
//! Endpoint: `POST /weapi/cloudsearch/get/web`
//!
//! Request parameters (encrypted):
//! - `s` — search keyword
//! - `type` — search type (1=track, 10=album, 100=artist, 1000=playlist)
//! - `limit` — page size (default 20, max 100)
//! - `offset` — pagination offset (0-based)
//!
//! Response JSON:
//! ```json
//! {
//!   "code": 200,
//!   "result": {
//!     "songCount": 268,          // when type=1
//!     "songs": [ { "id": 123, "name": "...", "ar": [...], "al": {...}, "dt": 240000 } ],
//!     "albumCount": 5,           // when type=10
//!     "albums": [ { "id": 456, "name": "...", "picUrl": "..." } ],
//!     "artistCount": 3,          // when type=100
//!     "artists": [ { "id": 789, "name": "..." } ],
//!     "playlistCount": 12,       // when type=1000
//!     "playlists": [ { "id": 101, "name": "...", "trackCount": 50, ... } ]
//!   }
//! }
//! ```

use crate::client::NeteaseClient;
use crate::error::Result;
use crate::types::{Album, Artist, Playlist, SearchResult, SearchType, Track, UserBrief};
use serde_json::{Value, json};

impl NeteaseClient {
    /// Search for tracks, albums, artists, or playlists.
    ///
    /// Returns a [`SearchResult`] with exactly one populated field matching
    /// the requested [`SearchType`]. Use `offset` for pagination.
    ///
    /// # Errors
    ///
    /// - [`NeteaseError::Http`] — network failure
    /// - [`NeteaseError::Api`] — server-side error (e.g. rate limit)
    pub fn search(
        &self,
        keyword: &str,
        search_type: SearchType,
        limit: u64,
        offset: u64,
    ) -> Result<SearchResult> {
        let data = json!({
            "s": keyword,
            "type": search_type as u64,
            "limit": limit,
            "offset": offset,
        });
        let resp = self.request("/cloudsearch/get/web", &data)?;
        let result = &resp["result"];

        let mut sr = SearchResult {
            total: 0,
            offset,
            limit,
            tracks: None,
            albums: None,
            playlists: None,
            artists: None,
        };

        match search_type {
            SearchType::Track => {
                sr.total = result["songCount"].as_u64().unwrap_or(0);
                sr.tracks = Some(parse_tracks(result["songs"].as_array()));
            }
            SearchType::Album => {
                sr.total = result["albumCount"].as_u64().unwrap_or(0);
                sr.albums = Some(parse_albums(result["albums"].as_array()));
            }
            SearchType::Artist => {
                sr.total = result["artistCount"].as_u64().unwrap_or(0);
                sr.artists = Some(parse_artists(result["artists"].as_array()));
            }
            SearchType::Playlist => {
                sr.total = result["playlistCount"].as_u64().unwrap_or(0);
                sr.playlists = Some(parse_playlists(result["playlists"].as_array()));
            }
        }

        Ok(sr)
    }
}

fn parse_tracks(arr: Option<&Vec<Value>>) -> Vec<Track> {
    let Some(arr) = arr else { return vec![] };
    arr.iter()
        .map(|v| {
            let artists = v["ar"]
                .as_array()
                .or_else(|| v["artists"].as_array())
                .map(|a| {
                    a.iter()
                        .map(|x| Artist {
                            id: x["id"].as_u64().unwrap_or(0),
                            name: x["name"].as_str().unwrap_or("").to_owned(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let al = if v["al"].is_null() {
                &v["album"]
            } else {
                &v["al"]
            };
            Track {
                id: v["id"].as_u64().unwrap_or(0),
                name: v["name"].as_str().unwrap_or("").to_owned(),
                artists,
                album: Album {
                    id: al["id"].as_u64().unwrap_or(0),
                    name: al["name"].as_str().unwrap_or("").to_owned(),
                    pic_url: al["picUrl"].as_str().map(String::from),
                },
                duration_ms: v["dt"].as_u64().unwrap_or(0),
            }
        })
        .collect()
}

fn parse_albums(arr: Option<&Vec<Value>>) -> Vec<Album> {
    let Some(arr) = arr else { return vec![] };
    arr.iter()
        .map(|v| Album {
            id: v["id"].as_u64().unwrap_or(0),
            name: v["name"].as_str().unwrap_or("").to_owned(),
            pic_url: v["picUrl"].as_str().map(String::from),
        })
        .collect()
}

fn parse_artists(arr: Option<&Vec<Value>>) -> Vec<Artist> {
    let Some(arr) = arr else { return vec![] };
    arr.iter()
        .map(|v| Artist {
            id: v["id"].as_u64().unwrap_or(0),
            name: v["name"].as_str().unwrap_or("").to_owned(),
        })
        .collect()
}

fn parse_playlists(arr: Option<&Vec<Value>>) -> Vec<Playlist> {
    let Some(arr) = arr else { return vec![] };
    arr.iter()
        .map(|v| {
            let creator = if v["creator"].is_null() {
                None
            } else {
                Some(UserBrief {
                    id: v["creator"]["userId"].as_u64().unwrap_or(0),
                    name: v["creator"]["nickname"].as_str().unwrap_or("").to_owned(),
                })
            };
            Playlist {
                id: v["id"].as_u64().unwrap_or(0),
                name: v["name"].as_str().unwrap_or("").to_owned(),
                description: v["description"].as_str().map(String::from),
                cover_url: v["coverImgUrl"].as_str().map(String::from),
                track_count: v["trackCount"].as_u64().unwrap_or(0),
                creator,
                tracks: None,
            }
        })
        .collect()
}
