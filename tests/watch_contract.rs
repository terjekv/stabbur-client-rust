//! Fault-injected transport contracts for deadlines and resumable terminal evidence.
#![cfg(all(feature = "async", feature = "blocking"))]
use futures_util::StreamExt;
use stabbur_client::{ApiError, Client, RunEvent, RunId, SecretToken, TerminalRunState, blocking};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
const RUN: &str = "01900000-0000-7000-8000-000000000001";
fn token() -> SecretToken {
    SecretToken::new("test-only-credential").unwrap()
}
fn fixture(bodies: Vec<String>, stall: Duration) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        for body in bodies {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "expected connection");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut byte = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                assert!(bytes.len() < 16384);
                stream.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
            }
            requests.push(String::from_utf8(bytes).unwrap());
            thread::sleep(stall);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
        requests
    });
    (format!("http://{address}"), worker)
}
fn log(sequence: u64) -> String {
    format!(
        "id: {sequence}\nevent: log\ndata: {{\"attempt_id\":\"01900000-0000-7000-8000-000000000002\",\"sequence\":{sequence},\"stream\":\"stdout\",\"message_base64\":\"b2s=\",\"occurred_at\":\"2026-01-01T00:00:00Z\"}}\n\n"
    )
}
fn complete(state: &str) -> String {
    format!("event: complete\ndata: {{\"run_id\":\"{RUN}\",\"state\":\"{state}\"}}\n\n")
}
#[test]
fn blocking_watch_reconnects_at_last_delivered_sequence_and_preserves_failure() {
    let (url, server) = fixture(
        vec![
            log(1),
            format!("{}{}{}", log(1), log(2), complete("failed")),
        ],
        Duration::ZERO,
    );
    let client = blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(token());
    let events = client
        .runs()
        .watch(RunId::parse(RUN).unwrap(), None, Duration::from_secs(3))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0],RunEvent::Log(log) if log.sequence==1));
    assert!(matches!(&events[1],RunEvent::Log(log) if log.sequence==2));
    assert!(
        matches!(&events[2],RunEvent::Complete(value) if value.state()==TerminalRunState::Failed)
    );
    let requests = server.join().unwrap();
    assert!(
        requests[1]
            .to_ascii_lowercase()
            .contains("last-event-id: 1\r\n")
    );
}
#[tokio::test]
async fn async_watch_reconnects_and_preserves_cancellation() {
    let (url, server) = fixture(
        vec![log(3), format!("{}{}", log(3), complete("cancelled"))],
        Duration::ZERO,
    );
    let client = Client::from_url(&url).unwrap().authenticate(token());
    let mut stream = client
        .runs()
        .watch(RUN.parse().unwrap(), None, Duration::from_secs(3))
        .into_stream();
    assert!(matches!(stream.next().await.unwrap().unwrap(),RunEvent::Log(log) if log.sequence==3));
    assert!(
        matches!(stream.next().await.unwrap().unwrap(),RunEvent::Complete(value) if value.state()==TerminalRunState::Cancelled)
    );
    assert!(stream.next().await.is_none());
    assert!(
        server.join().unwrap()[1]
            .to_ascii_lowercase()
            .contains("last-event-id: 3\r\n")
    );
}
#[test]
fn an_eof_never_proves_success() {
    let (url, server) = fixture(vec![String::new(); 4], Duration::ZERO);
    let client = blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(token());
    let outcome = client
        .runs()
        .watch(RUN.parse().unwrap(), None, Duration::from_secs(3))
        .next()
        .unwrap();
    assert!(matches!(outcome, Err(ApiError::EventStreamInterrupted)));
    server.join().unwrap();
}
#[tokio::test]
async fn a_stalled_request_respects_the_watch_deadline() {
    let (url, server) = fixture(vec![complete("succeeded")], Duration::from_millis(300));
    let client = Client::from_url(&url).unwrap().authenticate(token());
    let start = std::time::Instant::now();
    let result = client
        .runs()
        .watch(RUN.parse().unwrap(), None, Duration::from_millis(40))
        .into_stream()
        .next()
        .await
        .unwrap();
    assert!(matches!(result, Err(ApiError::WaitTimeout)));
    assert!(start.elapsed() < Duration::from_millis(250));
    server.join().unwrap();
}
