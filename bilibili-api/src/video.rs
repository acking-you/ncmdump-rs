//! Video detail and DASH audio stream APIs.

use crate::client::BilibiliClient;
use crate::error::{BilibiliError, Result};
use crate::types::{DashAudio, DashInfo, UserInfo, VideoDetail};
use reqwest::Url;

fn parse_av_id(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();
    let raw = lower.strip_prefix("av").unwrap_or(&lower);
    if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok()
}

fn extract_video_id_from_url(input: &str) -> Option<String> {
    let url = Url::parse(input).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !host.contains("bilibili.com") {
        return None;
    }

    let segments: Vec<_> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    for window in segments.windows(2) {
        if window[0] == "video" {
            return Some(window[1].to_string());
        }
    }
    None
}

impl BilibiliClient {
    /// Resolve user input into a BV ID.
    ///
    /// Accepts:
    /// - `BV...`
    /// - `av123456`
    /// - `https://www.bilibili.com/video/BV...`
    /// - `https://www.bilibili.com/video/av123456/`
    pub fn resolve_bvid(&self, input: &str) -> Result<String> {
        let candidate = input.trim();

        if candidate.is_empty() {
            return Err(BilibiliError::Other("empty Bilibili video id".into()));
        }

        let id = extract_video_id_from_url(candidate).unwrap_or_else(|| candidate.to_string());
        if id.starts_with("BV") {
            return Ok(id);
        }

        if let Some(aid) = parse_av_id(&id) {
            let detail = self.video_detail_by_aid(aid)?;
            return Ok(detail.bvid);
        }

        Err(BilibiliError::Other(format!(
            "unsupported Bilibili video id: {candidate}"
        )))
    }

    /// Get video detail by BV ID.
    pub fn video_detail(&self, bvid: &str) -> Result<VideoDetail> {
        let params = vec![("bvid".into(), bvid.to_owned())];
        let resp = self.wbi_get("/x/web-interface/view", &params)?;
        let data = &resp["data"];
        serde_json::from_value(data.clone())
            .map_err(|e| BilibiliError::Other(format!("parse video detail: {e}")))
    }

    /// Get video detail by AV ID.
    pub fn video_detail_by_aid(&self, aid: u64) -> Result<VideoDetail> {
        let params = vec![("aid".into(), aid.to_string())];
        let resp = self.wbi_get("/x/web-interface/view", &params)?;
        let data = &resp["data"];
        serde_json::from_value(data.clone())
            .map_err(|e| BilibiliError::Other(format!("parse video detail: {e}")))
    }

    /// Get DASH audio streams for a video.
    ///
    /// `fnval=4048` requests DASH format with all available audio codecs.
    pub fn dash_audio(&self, bvid: &str, cid: u64) -> Result<DashInfo> {
        let params = vec![
            ("bvid".into(), bvid.to_owned()),
            ("cid".into(), cid.to_string()),
            ("fnval".into(), "4048".into()),
            ("fnver".into(), "0".into()),
            ("fourk".into(), "1".into()),
        ];

        let resp = self.wbi_get("/x/player/wbi/playurl", &params)?;
        let dash = &resp["data"]["dash"];

        let audio: Vec<DashAudio> = dash["audio"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        let flac = dash["flac"]
            .as_object()
            .and_then(|obj| serde_json::from_value(serde_json::Value::Object(obj.clone())).ok());

        Ok(DashInfo { audio, flac })
    }

    /// Select the best audio stream from DASH info.
    ///
    /// Priority: FLAC (if available) > highest bandwidth AAC.
    pub fn best_audio(dash: &DashInfo) -> Option<&DashAudio> {
        // Try FLAC first.
        if let Some(flac) = &dash.flac {
            if flac.display {
                if let Some(audio) = &flac.audio {
                    return Some(audio);
                }
            }
        }
        // Fall back to highest bandwidth AAC.
        dash.audio.iter().max_by_key(|a| a.bandwidth)
    }

    /// Get current user info (nav API).
    pub fn user_info(&self) -> Result<UserInfo> {
        let resp = self.wbi_get("/x/web-interface/nav", &[])?;
        let data = &resp["data"];
        Ok(UserInfo {
            is_login: data["isLogin"].as_bool().unwrap_or(false),
            mid: data["mid"].as_u64().unwrap_or(0),
            name: data["uname"].as_str().unwrap_or("").to_owned(),
            face: data["face"].as_str().unwrap_or("").to_owned(),
            vip_status: data["vipStatus"].as_u64().unwrap_or(0),
        })
    }
}
