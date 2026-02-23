//! Data types for Bilibili API responses.

use serde::{Deserialize, Serialize};

/// Audio quality levels for DASH streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioQuality {
    /// 64 kbps AAC.
    Low = 30216,
    /// 132 kbps AAC.
    Normal = 30232,
    /// 192 kbps AAC (login required).
    High = 30280,
    /// Dolby Atmos (大会员).
    Dolby = 30250,
    /// Hi-Res FLAC (大会员).
    HiRes = 30251,
}

/// Output format for downloaded audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    Flac,
}

impl AudioFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
        }
    }
}

/// A video item from search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoItem {
    /// BV ID (e.g. `BV1xx411c7mD`).
    pub bvid: String,
    /// Video title (may contain HTML highlight tags).
    pub title: String,
    /// Video description.
    #[serde(default)]
    pub description: String,
    /// Author name.
    #[serde(default)]
    pub author: String,
    /// Author mid (user ID).
    #[serde(default)]
    pub mid: u64,
    /// Cover image URL.
    #[serde(default)]
    pub pic: String,
    /// Duration string (e.g. "4:32").
    #[serde(default)]
    pub duration: String,
    /// Play count.
    #[serde(default)]
    pub play: u64,
}

/// Video detail from `/x/web-interface/view`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDetail {
    pub bvid: String,
    /// AV ID.
    pub aid: u64,
    /// First part cid (for single-part videos).
    pub cid: u64,
    /// Video title.
    pub title: String,
    /// Cover image URL.
    pub pic: String,
    /// Description.
    #[serde(default)]
    pub desc: String,
    /// Duration in seconds.
    #[serde(default)]
    pub duration: u64,
    /// Author info.
    #[serde(default)]
    pub owner: VideoOwner,
    /// Video parts.
    #[serde(default)]
    pub pages: Vec<VideoPart>,
}

/// Video owner (uploader).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoOwner {
    pub mid: u64,
    #[serde(default)]
    pub name: String,
}

/// A single part/page of a multi-part video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoPart {
    pub cid: u64,
    pub page: u64,
    #[serde(default)]
    pub part: String,
    /// Duration in seconds.
    #[serde(default)]
    pub duration: u64,
}

/// A single DASH audio stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashAudio {
    /// Stream quality ID.
    pub id: u64,
    /// Stream URL.
    #[serde(default)]
    pub base_url: String,
    /// Backup URLs.
    #[serde(default)]
    pub backup_url: Vec<String>,
    /// Bandwidth in bps.
    #[serde(default)]
    pub bandwidth: u64,
    /// Codec string (e.g. "mp4a.40.2").
    #[serde(default)]
    pub codecs: String,
    /// MIME type.
    #[serde(default)]
    pub mime_type: String,
}

/// DASH stream info from playurl API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashInfo {
    /// Audio streams.
    #[serde(default)]
    pub audio: Vec<DashAudio>,
    /// FLAC audio stream (大会员).
    pub flac: Option<DashFlac>,
}

/// FLAC stream wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashFlac {
    pub display: bool,
    pub audio: Option<DashAudio>,
}

/// Search result wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Total result count.
    #[serde(default, alias = "numResults")]
    pub num_results: u64,
    /// Current page.
    #[serde(default)]
    pub page: u64,
    /// Page size.
    #[serde(default, alias = "pagesize")]
    pub page_size: u64,
    /// Video results.
    #[serde(default, alias = "result")]
    pub results: Vec<VideoItem>,
}

/// Current user info from `/x/web-interface/nav`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Whether logged in.
    #[serde(default, alias = "isLogin")]
    pub is_login: bool,
    /// User mid.
    #[serde(default)]
    pub mid: u64,
    /// Username.
    #[serde(default, alias = "uname")]
    pub name: String,
    /// Avatar URL.
    #[serde(default)]
    pub face: String,
    /// VIP status.
    #[serde(default)]
    pub vip_status: u64,
}
