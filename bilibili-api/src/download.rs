//! Audio download pipeline: DASH stream download + ffmpeg conversion.

use crate::client::BilibiliClient;
use crate::error::{BilibiliError, Result};
use crate::types::AudioFormat;
use std::path::Path;
use std::process::Command;

impl BilibiliClient {
    /// Download audio from a Bilibili video.
    ///
    /// Pipeline:
    /// 1. Get video detail → cid
    /// 2. Get DASH audio streams
    /// 3. Select best audio stream
    /// 4. Download raw m4s to temp file
    /// 5. Convert with ffmpeg to target format
    /// 6. Clean up temp file
    pub fn download_audio(
        &self,
        bvid: &str,
        output: &Path,
        format: AudioFormat,
    ) -> Result<u64> {
        let detail = self.video_detail(bvid)?;
        let cid = detail.cid;

        let dash = self.dash_audio(bvid, cid)?;
        let stream = Self::best_audio(&dash)
            .ok_or_else(|| BilibiliError::Other("no audio stream available".into()))?;

        let url = if stream.base_url.is_empty() {
            stream.backup_url.first()
                .ok_or_else(|| BilibiliError::Other("no audio URL".into()))?
                .as_str()
        } else {
            &stream.base_url
        };

        // Download raw m4s to temp file.
        let tmp_dir = std::env::temp_dir();
        let tmp_file = tmp_dir.join(format!("bili_{bvid}.m4s"));
        self.download_raw(url, &tmp_file)?;

        // Convert with ffmpeg.
        ffmpeg_convert(&tmp_file, output, format)?;

        // Clean up.
        let _ = std::fs::remove_file(&tmp_file);

        let size = std::fs::metadata(output)?.len();
        Ok(size)
    }
}

/// Convert a raw m4s/audio file to mp3 or flac using ffmpeg.
pub fn ffmpeg_convert(input: &Path, output: &Path, format: AudioFormat) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let args: Vec<&str> = match format {
        AudioFormat::Mp3 => vec![
            "-y", "-i", input.to_str().unwrap_or(""),
            "-codec:a", "libmp3lame", "-b:a", "320k",
            output.to_str().unwrap_or(""),
        ],
        AudioFormat::Flac => vec![
            "-y", "-i", input.to_str().unwrap_or(""),
            "-codec:a", "flac",
            output.to_str().unwrap_or(""),
        ],
    };

    let status = Command::new("ffmpeg")
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status()
        .map_err(|e| BilibiliError::Ffmpeg(format!("failed to run ffmpeg: {e}")))?;

    if !status.success() {
        return Err(BilibiliError::Ffmpeg(format!(
            "ffmpeg exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}

/// Check if ffmpeg is available in PATH.
pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
