use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct NcmMetadata {
    #[serde(rename = "musicName")]
    pub music_name: String,
    pub album: String,
    pub artist: Vec<Vec<serde_json::Value>>,
    pub bitrate: u64,
    pub duration: u64,
    pub format: String,
}

impl NcmMetadata {
    /// Parse metadata from the decrypted JSON bytes (after "music:" prefix is stripped).
    pub fn from_decrypted(data: &[u8]) -> Result<Self> {
        // Strip "music:" prefix if present
        let json_bytes = if data.starts_with(b"music:") {
            &data[6..]
        } else {
            data
        };
        Ok(serde_json::from_slice(json_bytes)?)
    }

    /// Join artist names with " / ".
    pub fn artist_names(&self) -> String {
        self.artist
            .iter()
            .filter_map(|a| a.first().and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_metadata() {
        let json = br#"{"musicName":"Test","album":"Album","artist":[["Artist1",0],["Artist2",1]],"bitrate":320000,"duration":240000,"format":"mp3"}"#;
        let meta = NcmMetadata::from_decrypted(json).unwrap();
        assert_eq!(meta.music_name, "Test");
        assert_eq!(meta.artist_names(), "Artist1 / Artist2");
    }

    #[test]
    fn test_parse_with_music_prefix() {
        let mut data = b"music:".to_vec();
        data.extend_from_slice(br#"{"musicName":"X","album":"A","artist":[],"bitrate":128000,"duration":1000,"format":"flac"}"#);
        let meta = NcmMetadata::from_decrypted(&data).unwrap();
        assert_eq!(meta.music_name, "X");
    }
}
