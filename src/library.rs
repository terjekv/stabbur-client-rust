//! Searchable software summaries shared by asynchronous and blocking transports.
use crate::{ApiError, ReleaseId, RunId, SoftwareChannelStatus, SoftwareId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Server-evaluated current attention views.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryView {
    /// All matching applications.
    #[default]
    All,
    /// Failed checks, unavailable workers, or releases awaiting review.
    Attention,
    /// Latest check failed with no replacement build outstanding.
    Failed,
    /// An enabled target lacks a recent compatible worker.
    Blocked,
    /// Available candidate releases await review.
    Review,
    /// No build has ever been created.
    NotBuilt,
}
/// Ordering never interprets opaque release versions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibrarySort {
    /// Display name in code-point order, then stable identity.
    #[default]
    Name,
    /// Most recently created identity first.
    Newest,
}
/// A validated literal name/slug search and attention predicate.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryQuery {
    q: String,
    view: LibraryView,
    sort: LibrarySort,
}
impl LibraryQuery {
    /// Accepts at most 200 UTF-8 bytes without controls. Wildcards remain literal.
    pub fn new(search: &str, view: LibraryView, sort: LibrarySort) -> Result<Self, ApiError> {
        if search.len() > 200 || search.chars().any(char::is_control) {
            return Err(ApiError::InvalidValue {
                kind: "library search",
                detail: "expected at most 200 bytes without control characters",
            });
        }
        Ok(Self {
            q: search.trim().to_owned(),
            view,
            sort,
        })
    }
}
/// Small software summary; publication does not attest device installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// Software identity.
    pub id: SoftwareId,
    /// Stable slug.
    pub slug: String,
    /// Display name.
    pub name: String,
    /// Software concurrency revision.
    pub revision: u64,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Current channel selections.
    pub channels: Vec<SoftwareChannelStatus>,
    /// Most recent run.
    pub latest_run_id: Option<RunId>,
    /// Most recent run state.
    pub latest_run_state: Option<String>,
    /// Last successful check.
    pub last_success_at: Option<DateTime<Utc>>,
    /// Next recurring check.
    pub next_run_at: Option<DateTime<Utc>>,
    /// Queued or running builds.
    pub outstanding_runs: u64,
    /// Enabled targets lacking a matching worker.
    pub blocked_targets: u64,
    /// Available candidate releases awaiting review.
    pub review_count: u64,
    /// Most recently created candidate.
    pub review_release_id: Option<ReleaseId>,
}
