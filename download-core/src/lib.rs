use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadProgressPhase {
    Queued,
    Preparing,
    ResolvingMeta,
    Downloading,
    PostProcessing,
    EmbeddingCover,
    SavingLyrics,
    RefreshingLibrary,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    pub job_id: String,
    pub source: String,
    pub phase: DownloadProgressPhase,
    pub percent: Option<u8>,
    pub message: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressSnapshot {
    pub job_id: String,
    pub source: String,
    pub state: String,
    pub phase: DownloadProgressPhase,
    pub percent: Option<u8>,
    pub message: String,
    pub detail: Option<String>,
    pub filename: Option<String>,
    pub warning: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadArtifact {
    pub path: PathBuf,
    pub filename: Option<String>,
    pub bytes_written: u64,
}

pub trait DownloadProgressReporter: Send + Sync {
    fn emit(&self, event: DownloadProgressEvent);
}

pub trait DownloadSource<Request> {
    type Error;

    fn download_with_progress(
        &self,
        request: &Request,
        target_path: &Path,
        reporter: Arc<dyn DownloadProgressReporter>,
    ) -> Result<DownloadArtifact, Self::Error>;
}

#[derive(Debug, Default)]
pub struct NoopProgressReporter;

impl DownloadProgressReporter for NoopProgressReporter {
    fn emit(&self, _event: DownloadProgressEvent) {}
}

pub fn snapshot_state(phase: &DownloadProgressPhase) -> &'static str {
    match phase {
        DownloadProgressPhase::Queued => "queued",
        DownloadProgressPhase::Completed => "completed",
        DownloadProgressPhase::Failed => "failed",
        DownloadProgressPhase::Preparing
        | DownloadProgressPhase::ResolvingMeta
        | DownloadProgressPhase::Downloading
        | DownloadProgressPhase::PostProcessing
        | DownloadProgressPhase::EmbeddingCover
        | DownloadProgressPhase::SavingLyrics
        | DownloadProgressPhase::RefreshingLibrary => "running",
    }
}

pub fn apply_progress_event(
    snapshot: Option<DownloadProgressSnapshot>,
    event: DownloadProgressEvent,
) -> DownloadProgressSnapshot {
    let mut snapshot = snapshot.unwrap_or_else(|| DownloadProgressSnapshot {
        job_id: event.job_id.clone(),
        source: event.source.clone(),
        state: snapshot_state(&event.phase).to_string(),
        phase: event.phase.clone(),
        percent: event.percent,
        message: event.message.clone(),
        detail: event.detail.clone(),
        filename: None,
        warning: None,
        error: None,
    });

    snapshot.job_id = event.job_id;
    snapshot.source = event.source;
    snapshot.state = snapshot_state(&event.phase).to_string();
    snapshot.phase = event.phase.clone();
    snapshot.percent = event.percent;
    snapshot.message = event.message;
    snapshot.detail = event.detail;

    if snapshot.phase == DownloadProgressPhase::Failed && snapshot.error.is_none() {
        snapshot.error = Some(snapshot.message.clone());
    }

    snapshot
}

#[cfg(test)]
mod tests {
    use super::{
        DownloadProgressEvent, DownloadProgressPhase, apply_progress_event, snapshot_state,
    };

    #[test]
    fn snapshot_state_maps_terminal_states() {
        assert_eq!(snapshot_state(&DownloadProgressPhase::Queued), "queued");
        assert_eq!(
            snapshot_state(&DownloadProgressPhase::Completed),
            "completed"
        );
        assert_eq!(snapshot_state(&DownloadProgressPhase::Failed), "failed");
        assert_eq!(
            snapshot_state(&DownloadProgressPhase::Downloading),
            "running"
        );
    }

    #[test]
    fn apply_progress_event_populates_snapshot() {
        let snapshot = apply_progress_event(
            None,
            DownloadProgressEvent {
                job_id: "job-1".into(),
                source: "netease".into(),
                phase: DownloadProgressPhase::Downloading,
                percent: Some(42),
                message: "Downloading audio".into(),
                detail: Some("123 / 456 bytes".into()),
            },
        );

        assert_eq!(snapshot.state, "running");
        assert_eq!(snapshot.percent, Some(42));
        assert_eq!(snapshot.detail.as_deref(), Some("123 / 456 bytes"));
    }
}
