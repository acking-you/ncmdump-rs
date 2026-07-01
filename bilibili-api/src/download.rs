//! Audio download pipeline: DASH stream download with optional ffmpeg conversion.

use crate::client::BilibiliClient;
use crate::error::{BilibiliError, Result};
use crate::types::AudioFormat;
use download_core::{
    DownloadArtifact, DownloadProgressEvent, DownloadProgressPhase, DownloadProgressReporter,
    DownloadSource, NoopProgressReporter,
};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DownloadAudioRequest {
    pub job_id: String,
    pub bvid: String,
}

#[derive(Debug, Clone)]
pub struct DownloadAudioTranscodeRequest {
    pub job_id: String,
    pub bvid: String,
    pub format: AudioFormat,
}

impl BilibiliClient {
    /// Download the best available raw audio stream from a Bilibili video.
    ///
    /// This skips transcoding and writes the selected DASH audio payload as-is.
    pub fn download_audio_raw(&self, bvid: &str, output: &Path) -> Result<u64> {
        let request = DownloadAudioRequest {
            job_id: "download".to_string(),
            bvid: bvid.to_string(),
        };
        let artifact = self.download_audio_raw_with_progress(
            &request,
            output,
            Arc::new(NoopProgressReporter),
        )?;
        Ok(artifact.bytes_written)
    }

    pub fn download_audio_raw_with_progress(
        &self,
        request: &DownloadAudioRequest,
        output: &Path,
        reporter: Arc<dyn DownloadProgressReporter>,
    ) -> Result<DownloadArtifact> {
        reporter.emit(DownloadProgressEvent {
            job_id: request.job_id.clone(),
            source: "bilibili".to_string(),
            phase: DownloadProgressPhase::Preparing,
            percent: None,
            message: format!("Preparing Bilibili video {}", request.bvid),
            detail: None,
        });
        reporter.emit(DownloadProgressEvent {
            job_id: request.job_id.clone(),
            source: "bilibili".to_string(),
            phase: DownloadProgressPhase::ResolvingMeta,
            percent: None,
            message: "Resolving audio stream".to_string(),
            detail: Some(request.bvid.clone()),
        });

        let detail = match self.video_detail(&request.bvid) {
            Ok(detail) => detail,
            Err(error) => {
                reporter.emit(DownloadProgressEvent {
                    job_id: request.job_id.clone(),
                    source: "bilibili".to_string(),
                    phase: DownloadProgressPhase::Failed,
                    percent: None,
                    message: "Failed to load video detail".to_string(),
                    detail: Some(error.to_string()),
                });
                return Err(error);
            }
        };
        let dash = match self.dash_audio(&request.bvid, detail.cid) {
            Ok(dash) => dash,
            Err(error) => {
                reporter.emit(DownloadProgressEvent {
                    job_id: request.job_id.clone(),
                    source: "bilibili".to_string(),
                    phase: DownloadProgressPhase::Failed,
                    percent: None,
                    message: "Failed to resolve DASH audio".to_string(),
                    detail: Some(error.to_string()),
                });
                return Err(error);
            }
        };
        let stream = match Self::best_audio(&dash) {
            Some(stream) => stream,
            None => {
                let error = BilibiliError::Other("no audio stream available".into());
                reporter.emit(DownloadProgressEvent {
                    job_id: request.job_id.clone(),
                    source: "bilibili".to_string(),
                    phase: DownloadProgressPhase::Failed,
                    percent: None,
                    message: "No audio stream available".to_string(),
                    detail: None,
                });
                return Err(error);
            }
        };

        let url = if stream.base_url.is_empty() {
            match stream.backup_url.first() {
                Some(url) => url.as_str(),
                None => {
                    let error = BilibiliError::Other("no audio URL".into());
                    reporter.emit(DownloadProgressEvent {
                        job_id: request.job_id.clone(),
                        source: "bilibili".to_string(),
                        phase: DownloadProgressPhase::Failed,
                        percent: None,
                        message: "No audio URL available".to_string(),
                        detail: None,
                    });
                    return Err(error);
                }
            }
        } else {
            &stream.base_url
        };

        let bytes_written = self.download_raw_with_progress(
            url,
            output,
            request.job_id.clone(),
            reporter.clone(),
            "bilibili".to_string(),
            "Downloading audio stream".to_string(),
        )?;
        let filename = output
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());

        reporter.emit(DownloadProgressEvent {
            job_id: request.job_id.clone(),
            source: "bilibili".to_string(),
            phase: DownloadProgressPhase::Completed,
            percent: Some(100),
            message: "Download complete".to_string(),
            detail: filename.clone(),
        });

        Ok(DownloadArtifact {
            path: output.to_path_buf(),
            filename,
            bytes_written,
        })
    }

    /// Download audio from a Bilibili video.
    ///
    /// Pipeline:
    /// 1. Get video detail → cid
    /// 2. Get DASH audio streams
    /// 3. Select best audio stream
    /// 4. Download raw m4s to temp file
    /// 5. Convert with ffmpeg to target format
    /// 6. Clean up temp file
    pub fn download_audio(&self, bvid: &str, output: &Path, format: AudioFormat) -> Result<u64> {
        let request = DownloadAudioTranscodeRequest {
            job_id: "download".to_string(),
            bvid: bvid.to_string(),
            format,
        };
        let artifact =
            self.download_audio_with_progress(&request, output, Arc::new(NoopProgressReporter))?;
        Ok(artifact.bytes_written)
    }

    pub fn download_audio_with_progress(
        &self,
        request: &DownloadAudioTranscodeRequest,
        output: &Path,
        reporter: Arc<dyn DownloadProgressReporter>,
    ) -> Result<DownloadArtifact> {
        // Download raw m4s to temp file.
        let tmp_dir = std::env::temp_dir();
        let tmp_file = tmp_dir.join(format!("bili_{}.m4s", request.bvid));
        self.download_audio_raw_with_progress(
            &DownloadAudioRequest {
                job_id: request.job_id.clone(),
                bvid: request.bvid.clone(),
            },
            &tmp_file,
            reporter.clone(),
        )?;

        // Convert with ffmpeg.
        reporter.emit(DownloadProgressEvent {
            job_id: request.job_id.clone(),
            source: "bilibili".to_string(),
            phase: DownloadProgressPhase::PostProcessing,
            percent: None,
            message: "Converting downloaded audio".to_string(),
            detail: Some(format!("format={}", request.format.extension())),
        });
        if let Err(error) = ffmpeg_convert(&tmp_file, output, request.format) {
            reporter.emit(DownloadProgressEvent {
                job_id: request.job_id.clone(),
                source: "bilibili".to_string(),
                phase: DownloadProgressPhase::Failed,
                percent: None,
                message: "Audio conversion failed".to_string(),
                detail: Some(error.to_string()),
            });
            return Err(error);
        }

        // Clean up.
        let _ = std::fs::remove_file(&tmp_file);

        let size = std::fs::metadata(output)?.len();
        let filename = output
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        reporter.emit(DownloadProgressEvent {
            job_id: request.job_id.clone(),
            source: "bilibili".to_string(),
            phase: DownloadProgressPhase::Completed,
            percent: Some(100),
            message: "Download complete".to_string(),
            detail: filename.clone(),
        });
        Ok(DownloadArtifact {
            path: output.to_path_buf(),
            filename,
            bytes_written: size,
        })
    }
}

impl DownloadSource<DownloadAudioRequest> for BilibiliClient {
    type Error = BilibiliError;

    fn download_with_progress(
        &self,
        request: &DownloadAudioRequest,
        target_path: &Path,
        reporter: Arc<dyn DownloadProgressReporter>,
    ) -> std::result::Result<DownloadArtifact, Self::Error> {
        self.download_audio_raw_with_progress(request, target_path, reporter)
    }
}

/// Convert a raw m4s/audio file to mp3 or flac using ffmpeg.
pub fn ffmpeg_convert(input: &Path, output: &Path, format: AudioFormat) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let args: Vec<&str> = match format {
        AudioFormat::Mp3 => vec![
            "-y",
            "-i",
            input.to_str().unwrap_or(""),
            "-codec:a",
            "libmp3lame",
            "-b:a",
            "320k",
            output.to_str().unwrap_or(""),
        ],
        AudioFormat::Flac => vec![
            "-y",
            "-i",
            input.to_str().unwrap_or(""),
            "-codec:a",
            "flac",
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
