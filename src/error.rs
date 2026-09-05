use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One field-level server validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Request field.
    pub field: String,
    /// Stable machine-readable code.
    pub code: String,
    /// Safe detail.
    pub message: String,
}

/// RFC 9457-style Stabbur error body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    /// Stable Stabbur error code.
    pub code: String,
    /// HTTP status.
    pub status: u16,
    /// Safe human-readable detail.
    pub detail: String,
    /// Correlation identity.
    pub request_id: String,
    /// Optional validation details.
    #[serde(default)]
    pub validation_errors: Vec<ValidationError>,
}

/// Public client failure with credentials and backend paths excluded.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Base URL violates client safety policy.
    #[error("invalid Stabbur base URL: {0}")]
    InvalidBaseUrl(&'static str),
    /// A typed ID, digest, or request value is invalid.
    #[error("invalid {kind}: {detail}")]
    InvalidValue {
        /// Value kind.
        kind: &'static str,
        /// Safe validation detail.
        detail: &'static str,
    },
    /// The server returned `application/problem+json`.
    #[error("Stabbur request failed: {0:?}")]
    Server(Problem),
    /// The server returned an unexpected status without a problem body.
    #[error("Stabbur request failed with HTTP status {status} (request {request_id:?})")]
    HttpStatus {
        /// HTTP status.
        status: u16,
        /// Optional correlation identity.
        request_id: Option<String>,
    },
    /// Network operation failed. The underlying URL and headers are deliberately omitted.
    #[error("Stabbur transport failed")]
    Transport,
    /// A successful response did not match the released contract.
    #[error("Stabbur response did not match the released contract")]
    Decode,
    /// A buffered response exceeded the public client's memory limit.
    #[error("Stabbur response exceeded the 8 MiB limit")]
    ResponseTooLarge,
    /// A bounded download could not be written.
    #[error("artifact download output failed")]
    DownloadOutput,
    /// Reviewed desired changes no longer match current server state.
    #[error("catalog plan is stale; review a new plan before applying")]
    StalePlan,
    /// The SSE stream violates its bounded, typed run contract.
    #[error("run event stream did not match the contract")]
    InvalidEventStream,
    /// The stream ended repeatedly before a terminal event.
    #[error("run event stream ended before completion")]
    EventStreamInterrupted,
    /// A run did not reach a terminal state before the caller's deadline.
    #[error("run wait deadline expired")]
    WaitTimeout,
}
