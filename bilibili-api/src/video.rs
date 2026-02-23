//! Video detail and DASH audio stream APIs.

use crate::client::BilibiliClient;
use crate::error::{BilibiliError, Result};
use crate::types::{DashAudio, DashInfo, UserInfo, VideoDetail};

impl BilibiliClient {
    /// Get video detail by BV ID.
    pub fn video_detail(&self, bvid: &str) -> Result<VideoDetail> {
        let params = vec![("bvid".into(), bvid.to_owned())];
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
