//! Data types for Netease Cloud Music API responses.
//!
//! These types are deserialized from the raw JSON returned by the WEAPI
//! endpoints. Field names follow Rust conventions (`snake_case`) rather than
//! the original API naming (camelCase).

use serde::{Deserialize, Serialize};

/// A music artist.
///
/// Returned inside [`Track`] and [`SearchResult`].
///
/// API JSON fields: `id` (number), `name` (string).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    /// Netease artist ID.
    pub id: u64,
    /// Display name.
    pub name: String,
}

/// An album.
///
/// Returned inside [`Track`] (as `al` or `album`) and in album search results.
///
/// API JSON fields: `id`, `name`, `picUrl` (optional cover image URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    /// Netease album ID.
    pub id: u64,
    /// Album title.
    pub name: String,
    /// Cover image URL (e.g. `https://p1.music.126.net/...`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pic_url: Option<String>,
}

/// A music track (song).
///
/// Returned by [`NeteaseClient::track_detail`](crate::NeteaseClient::track_detail)
/// and inside [`SearchResult`] / [`Playlist`].
///
/// API JSON fields: `id`, `name`, `ar`/`artists` (artist array),
/// `al`/`album` (album object), `dt`/`duration` (milliseconds).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Netease track ID (used in `track_url`, `track_lyric`, `download_track`).
    pub id: u64,
    /// Song title.
    pub name: String,
    /// Performing artists.
    pub artists: Vec<Artist>,
    /// Album this track belongs to.
    pub album: Album,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// A playlist (song list).
///
/// Returned by [`NeteaseClient::playlist_detail`](crate::NeteaseClient::playlist_detail)
/// and in playlist search results.
///
/// API JSON path: `response.playlist` (detail) or `response.result.playlists` (search).
///
/// Fields from API: `id`, `name`, `description`, `coverImgUrl`, `trackCount`,
/// `creator` (`{ userId, nickname }`), `tracks` (array, only in detail endpoint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    /// Netease playlist ID.
    pub id: u64,
    /// Playlist title.
    pub name: String,
    /// User-written description (may be absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Cover image URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    /// Total number of tracks in the playlist.
    pub track_count: u64,
    /// Playlist creator info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<UserBrief>,
    /// Full track list (only populated by `playlist_detail`, not by search).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<Vec<Track>>,
}

/// Abbreviated user info embedded in [`Playlist`].
///
/// API JSON fields: `userId` (number), `nickname` (string).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBrief {
    /// Netease user ID.
    pub id: u64,
    /// Display name.
    pub name: String,
}

/// Current user profile.
///
/// Returned by [`NeteaseClient::user_info`](crate::NeteaseClient::user_info).
///
/// API JSON path: `response.profile` with fields `userId`, `nickname`, `avatarUrl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// Netease user ID.
    pub id: u64,
    /// Display nickname.
    pub nickname: String,
    /// Avatar image URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

/// Song lyrics.
///
/// Returned by [`NeteaseClient::track_lyric`](crate::NeteaseClient::track_lyric).
///
/// API JSON path: `response.lrc.lyric` (original) and `response.tlyric.lyric` (translation).
/// Both are in LRC format (e.g. `[00:12.34]歌词内容`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyric {
    /// Original lyrics in LRC format. `None` if the track has no lyrics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lrc: Option<String>,
    /// Translated lyrics (usually Chinese ↔ other language). `None` if unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tlyric: Option<String>,
}

/// Paginated search results.
///
/// Returned by [`NeteaseClient::search`](crate::NeteaseClient::search).
///
/// Exactly one of `tracks`, `albums`, `playlists`, or `artists` will be `Some`,
/// depending on the [`SearchType`] used in the query.
///
/// API JSON path: `response.result` with type-specific arrays and counts
/// (`songCount`/`songs`, `albumCount`/`albums`, `artistCount`/`artists`,
/// `playlistCount`/`playlists`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Total number of matching results on the server.
    pub total: u64,
    /// Current page offset (0-based).
    pub offset: u64,
    /// Page size.
    pub limit: u64,
    /// Matched tracks (when `SearchType::Track`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<Vec<Track>>,
    /// Matched albums (when `SearchType::Album`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub albums: Option<Vec<Album>>,
    /// Matched playlists (when `SearchType::Playlist`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlists: Option<Vec<Playlist>>,
    /// Matched artists (when `SearchType::Artist`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artists: Option<Vec<Artist>>,
}

/// Search target type, mapped to the API `type` parameter.
///
/// | Variant    | API value | Searches for |
/// |------------|-----------|--------------|
/// | `Track`    | 1         | Songs        |
/// | `Album`    | 10        | Albums       |
/// | `Artist`   | 100       | Artists      |
/// | `Playlist` | 1000      | Playlists    |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchType {
    Track = 1,
    Album = 10,
    Artist = 100,
    Playlist = 1000,
}

/// Audio quality / bitrate for track playback URLs.
///
/// Passed to [`NeteaseClient::track_url`](crate::NeteaseClient::track_url) as the
/// `br` (bitrate) parameter. The server returns the best available quality up to
/// the requested level, subject to the user's VIP tier.
///
/// | Variant    | Bitrate   | Typical format |
/// |------------|-----------|----------------|
/// | `Standard` | 128 kbps  | MP3            |
/// | `Higher`   | 192 kbps  | MP3            |
/// | `Exhigh`   | 320 kbps  | MP3            |
/// | `Lossless` | 999 kbps* | FLAC           |
///
/// *999000 is a sentinel value; actual lossless bitrate varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    /// 128 kbps MP3.
    Standard,
    /// 192 kbps MP3.
    Higher,
    /// 320 kbps MP3.
    Exhigh,
    /// Lossless (FLAC). Requires VIP.
    Lossless,
}

impl Quality {
    /// Return the bitrate value sent to the API `br` parameter.
    pub fn bitrate(self) -> u64 {
        match self {
            Self::Standard => 128_000,
            Self::Higher => 192_000,
            Self::Exhigh => 320_000,
            Self::Lossless => 999_000,
        }
    }
}
