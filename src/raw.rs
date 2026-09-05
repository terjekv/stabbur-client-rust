use std::fmt;

use serde::de::DeserializeOwned;

use crate::ApiError;

const MAX_SEGMENTS: usize = 16;
const MAX_SEGMENT_BYTES: usize = 256;
const MAX_QUERY_ITEMS: usize = 32;
const MAX_QUERY_VALUE_BYTES: usize = 1024;
pub(crate) const MAX_RAW_REQUEST_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_RAW_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// HTTP method allowed by the constrained authenticated extension point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawMethod {
    /// Read a resource.
    Get,
    /// Inspect response metadata without a body.
    Head,
    /// Create or invoke a resource operation.
    Post,
    /// Replace a resource or stream content.
    Put,
    /// Partially update a resource.
    Patch,
    /// Delete or revoke a resource.
    Delete,
}

/// Checked request for a future public `/api/v1` operation.
#[derive(Clone)]
pub struct RawRequest {
    pub(crate) method: RawMethod,
    pub(crate) path: String,
    pub(crate) query: Vec<(String, String)>,
    pub(crate) body: Option<serde_json::Value>,
    pub(crate) revision: Option<u64>,
    pub(crate) idempotency_key: Option<String>,
}

impl RawRequest {
    /// Builds a request from opaque path segments below `/api/v1`.
    ///
    /// The internal worker protocol cannot be addressed through this API. Each supplied value is
    /// encoded as exactly one path segment, so traversal, query, and fragment injection are inert.
    pub fn new<I, S>(method: RawMethod, segments: I) -> Result<Self, ApiError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let segments = segments
            .into_iter()
            .map(|segment| segment.as_ref().to_owned())
            .collect::<Vec<_>>();
        if segments.is_empty() || segments.len() > MAX_SEGMENTS {
            return Err(ApiError::InvalidValue {
                kind: "raw path",
                detail: "expected 1 through 16 public API path segments",
            });
        }
        if segments[0].eq_ignore_ascii_case("internal") {
            return Err(ApiError::InvalidValue {
                kind: "raw path",
                detail: "the internal worker protocol is not a public client extension",
            });
        }
        if segments.iter().any(|segment| {
            segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.len() > MAX_SEGMENT_BYTES
        }) {
            return Err(ApiError::InvalidValue {
                kind: "raw path segment",
                detail: "segments must contain 1 through 256 bytes",
            });
        }
        let path = format!(
            "/api/v1/{}",
            segments
                .iter()
                .map(|segment| encode_segment(segment))
                .collect::<Vec<_>>()
                .join("/")
        );
        Ok(Self {
            method,
            path,
            query: Vec::new(),
            body: None,
            revision: None,
            idempotency_key: None,
        })
    }

    /// Adds one encoded query pair.
    pub fn query(mut self, key: &str, value: &str) -> Result<Self, ApiError> {
        if self.query.len() >= MAX_QUERY_ITEMS
            || key.is_empty()
            || key.len() > MAX_QUERY_VALUE_BYTES
            || value.len() > MAX_QUERY_VALUE_BYTES
            || key.bytes().any(|byte| byte.is_ascii_control())
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(ApiError::InvalidValue {
                kind: "raw query",
                detail: "expected at most 32 non-control key/value pairs of at most 1024 bytes",
            });
        }
        self.query.push((key.to_owned(), value.to_owned()));
        Ok(self)
    }

    /// Sets a JSON body. Serialized bodies are limited again immediately before transmission.
    #[must_use]
    pub fn json(mut self, body: serde_json::Value) -> Self {
        self.body = Some(body);
        self
    }

    /// Applies optimistic concurrency as `If-Match: "rev-N"`.
    #[must_use]
    pub fn revision(mut self, revision: u64) -> Self {
        self.revision = Some(revision);
        self
    }

    /// Applies an idempotency key to a retryable operation.
    pub fn idempotency_key(mut self, value: &str) -> Result<Self, ApiError> {
        if value.is_empty()
            || value.len() > 128
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(ApiError::InvalidValue {
                kind: "idempotency key",
                detail: "expected 1 through 128 bytes without control characters",
            });
        }
        self.idempotency_key = Some(value.to_owned());
        Ok(self)
    }
}

impl fmt::Debug for RawRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawRequest")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query_items", &self.query.len())
            .field("body", &self.body.as_ref().map(|_| "[PRESENT]"))
            .field("revision", &self.revision)
            .field(
                "idempotency_key",
                &self.idempotency_key.as_ref().map(|_| "[PRESENT]"),
            )
            .finish()
    }
}

/// Bounded response from the authenticated extension point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawResponse {
    /// HTTP success status.
    pub status: u16,
    /// Correlation identity when returned by the server.
    pub request_id: Option<String>,
    /// Response media type when returned by the server.
    pub content_type: Option<String>,
    /// Response bytes, limited to 8 MiB.
    pub body: Vec<u8>,
}

impl RawResponse {
    /// Decodes the bounded response body as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, ApiError> {
        serde_json::from_slice(&self.body).map_err(|_| ApiError::Decode)
    }
}

fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").expect("writing to a String is infallible");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_paths_are_encoded_and_internal_protocol_is_blocked() {
        let request = RawRequest::new(RawMethod::Get, ["software", "../safe?raw=true"])
            .expect("safe extension path");
        assert_eq!(request.path, "/api/v1/software/..%2Fsafe%3Fraw%3Dtrue");
        assert!(RawRequest::new(RawMethod::Post, ["internal", "workers"]).is_err());
    }

    #[test]
    fn raw_debug_redacts_body_and_idempotency_value() {
        let secret = "raw-body-secret";
        let request = RawRequest::new(RawMethod::Post, ["extension"])
            .unwrap()
            .json(serde_json::json!({"token": secret}))
            .idempotency_key("private-correlation-value")
            .unwrap();
        let rendered = format!("{request:?}");
        assert!(!rendered.contains(secret));
        assert!(!rendered.contains("private-correlation-value"));
    }
}
