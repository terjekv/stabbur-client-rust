//! Bounded, transport-independent parsing of durable run events.
use crate::{ApiError, RunId, RunLog};
use serde::{Deserialize, Serialize};

/// A terminal run outcome. A running/queued state cannot construct this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalRunState {
    /// The build completed successfully.
    Succeeded,
    /// The build failed.
    Failed,
    /// The build was cancelled.
    Cancelled,
}
/// Parsed terminal evidence with a validated identity and terminal-only state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunCompletion {
    run_id: RunId,
    state: TerminalRunState,
}
impl RunCompletion {
    /// Identity of the completed run.
    pub const fn run_id(&self) -> RunId {
        self.run_id
    }
    /// Terminal build outcome.
    pub const fn state(&self) -> TerminalRunState {
        self.state
    }
}
/// One validated event from the public run stream.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum RunEvent {
    /// Exact-byte log with a durable sequence used for reconnect/replay.
    Log(RunLog),
    /// A terminal run outcome.
    Complete(RunCompletion),
}

/// Incremental SSE decoder. Its bounded state and expected run identity cannot be changed.
pub struct RunEventDecoder {
    run: RunId,
    line: Vec<u8>,
    event: String,
    data: String,
    id: Option<u64>,
}
impl RunEventDecoder {
    /// Creates a parser tied to the requested run.
    pub fn new(run: RunId) -> Self {
        Self {
            run,
            line: Vec::new(),
            event: String::new(),
            data: String::new(),
            id: None,
        }
    }
    /// Feeds transport bytes, preserving partial UTF-8 and CRLF boundaries.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<RunEvent>, ApiError> {
        let mut events = Vec::new();
        for byte in bytes {
            if *byte == b'\n' {
                let line = std::mem::take(&mut self.line);
                let line = std::str::from_utf8(&line)
                    .map_err(|_| ApiError::InvalidEventStream)?
                    .trim_end_matches('\r');
                if line.is_empty() {
                    if self.data.is_empty() {
                        self.event.clear();
                        self.id = None;
                        continue;
                    }
                    let event = match self.event.as_str() {
                        "log" => {
                            let log: RunLog = serde_json::from_str(&self.data)
                                .map_err(|_| ApiError::InvalidEventStream)?;
                            if self.id != Some(log.sequence) {
                                return Err(ApiError::InvalidEventStream);
                            }
                            RunEvent::Log(log)
                        }
                        "complete" => {
                            let completion: RunCompletion = serde_json::from_str(&self.data)
                                .map_err(|_| ApiError::InvalidEventStream)?;
                            if completion.run_id != self.run {
                                return Err(ApiError::InvalidEventStream);
                            }
                            RunEvent::Complete(completion)
                        }
                        _ => return Err(ApiError::InvalidEventStream),
                    };
                    events.push(event);
                    self.event.clear();
                    self.data.clear();
                    self.id = None;
                } else if !line.starts_with(':') {
                    let (field, value) = line.split_once(':').unwrap_or((line, ""));
                    let value = value.strip_prefix(' ').unwrap_or(value);
                    match field {
                        "event" => {
                            if value.len() > 64 {
                                return Err(ApiError::InvalidEventStream);
                            }
                            value.clone_into(&mut self.event);
                        }
                        "id" => {
                            self.id =
                                Some(value.parse().map_err(|_| ApiError::InvalidEventStream)?);
                        }
                        "data" => {
                            if self.data.len() + value.len() + 1 > 1024 * 1024 {
                                return Err(ApiError::InvalidEventStream);
                            }
                            self.data.push_str(value);
                            self.data.push('\n');
                        }
                        _ => {}
                    }
                }
            } else {
                if self.line.len() >= 1024 * 1024 {
                    return Err(ApiError::InvalidEventStream);
                }
                self.line.push(*byte);
            }
        }
        Ok(events)
    }
}

pub(crate) struct WatchCursor {
    pub(crate) run: RunId,
    pub(crate) last: Option<u64>,
    decoder: RunEventDecoder,
    pending: std::collections::VecDeque<RunEvent>,
    pub(crate) finished: bool,
}
impl WatchCursor {
    pub(crate) fn new(run: RunId, last: Option<u64>) -> Self {
        Self {
            run,
            last,
            decoder: RunEventDecoder::new(run),
            pending: std::collections::VecDeque::new(),
            finished: false,
        }
    }
    pub(crate) fn reconnect(&mut self) {
        self.decoder = RunEventDecoder::new(self.run);
    }
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Result<(), ApiError> {
        self.pending.extend(self.decoder.feed(bytes)?);
        Ok(())
    }
    pub(crate) fn pop(&mut self) -> Option<RunEvent> {
        while let Some(event) = self.pending.pop_front() {
            match &event {
                RunEvent::Log(log) => {
                    if self.last.is_some_and(|last| log.sequence <= last) {
                        continue;
                    }
                    self.last = Some(log.sequence);
                }
                RunEvent::Complete(_) => self.finished = true,
            }
            return Some(event);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id() -> RunId {
        "01900000-0000-7000-8000-000000000001".parse().unwrap()
    }
    #[test]
    fn terminal_state_and_identity_are_validated_across_arbitrary_boundaries() {
        let value = format!(
            "event:complete\r\ndata:{{\"run_id\":\"{}\",\"state\":\"failed\"}}\r\n\r\n",
            id()
        );
        for split in 0..value.len() {
            let mut parser = RunEventDecoder::new(id());
            let mut events = parser.feed(&value.as_bytes()[..split]).unwrap();
            events.extend(parser.feed(&value.as_bytes()[split..]).unwrap());
            assert!(
                matches!(&events[..], [RunEvent::Complete(value)] if value.state() == TerminalRunState::Failed)
            );
        }
        let invalid = value.replace("failed", "running");
        assert!(RunEventDecoder::new(id()).feed(invalid.as_bytes()).is_err());
        let invalid = value.replace("8000-000000000001", "8000-000000000002");
        assert!(RunEventDecoder::new(id()).feed(invalid.as_bytes()).is_err());
    }
    #[test]
    fn unbounded_lines_are_rejected() {
        assert!(
            RunEventDecoder::new(id())
                .feed(&vec![b'a'; 1024 * 1024 + 1])
                .is_err()
        );
    }
}
