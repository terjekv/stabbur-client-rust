//! Transport-boundary tests using a one-request loopback mock server.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use stabbur_client::{
    BuildTargetSchedule, NewRecipeRevision, PinnedSource, RawMethod, RawRequest, RecipeRevisionId,
    SecretToken, Software,
};

fn mock_once(status: &str, body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let body = body.to_owned();
    let status = status.to_owned();
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    request.extend_from_slice(&buffer[..count]);
                    if complete_request(&request) {
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("mock read failed: {error}"),
            }
        }
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (format!("http://{address}"), task)
}

fn complete_request(request: &[u8]) -> bool {
    let Some(header_end) = request.windows(4).position(|value| value == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    request.len() >= header_end + 4 + content_length
}

fn mock_oversized_response() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let response = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 8388609\r\nconnection: close\r\n\r\n";
        stream.write_all(response.as_bytes()).unwrap();
    });
    (format!("http://{address}"), task)
}

fn software_json(revision: u64) -> String {
    format!(
        r#"{{"id":"01900000-0000-7000-8000-000000000001","slug":"safe","name":"Safe","created_at":"2026-01-01T00:00:00Z","revision":{revision},"installation":null}}"#
    )
}

fn fake_recipe_revision_json() -> &'static str {
    r#"{"id":"01900000-0000-7000-8000-000000000010","recipe_id":"01900000-0000-7000-8000-000000000011","sequence":1,"builder":"fake","definition":{},"required_capabilities":["builder.fake","runtime.portable"],"created_at":"2026-01-01T00:00:00Z"}"#
}

fn build_target_json() -> &'static str {
    r#"{"id":"01900000-0000-7000-8000-000000000020","name":"firefox-nightly","software_id":"01900000-0000-7000-8000-000000000001","recipe_revision_id":"01900000-0000-7000-8000-000000000010","parameters":{},"schedule":{"kind":"manual"},"enabled":true,"next_run_at":null,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","revision":1}"#
}

fn catalog_scan_json() -> &'static str {
    r#"{"id":"01900000-0000-7000-8000-000000000030","job_id":"01900000-0000-7000-8000-000000000031","producer":"autopkg","source":{"locator":"https://example.test/recipes.git","revision":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"state":"queued","snapshot_id":null,"failure":null,"requested_at":"2026-01-01T00:00:00Z","completed_at":null}"#
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_paths_are_encoded_and_credentials_stay_in_headers() {
    let (url, request) = mock_once("200 OK", &software_json(1));
    let client = stabbur_client::Client::from_url(&url)
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let software: Software = client.software().get("../admin?raw=true").await.unwrap();
    assert_eq!(software.slug, "safe");
    let request = request.join().unwrap();
    assert!(request.starts_with("GET /api/v1/software/..%2Fadmin%3Fraw%3Dtrue HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-secret-token\r\n")
    );
    assert!(
        !request
            .lines()
            .next()
            .unwrap()
            .contains("fixture-secret-token")
    );
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_mutation_sends_revision_etag_and_contract_json() {
    let (url, request) = mock_once("200 OK", &software_json(8));
    let client = stabbur_client::blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let software = client
        .software()
        .update_name("safe", "Replacement", 7)
        .unwrap();
    assert_eq!(software.revision, 8);
    let request = request.join().unwrap();
    assert!(request.starts_with("PATCH /api/v1/software/safe HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("if-match: \"rev-7\"\r\n")
    );
    assert!(request.ends_with(r#"{"name":"Replacement"}"#));
}

#[cfg(feature = "blocking")]
#[test]
fn builder_neutral_recipe_revision_uses_the_released_contract() {
    let (url, request) = mock_once("201 Created", fake_recipe_revision_json());
    let client = stabbur_client::blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let revision = client
        .recipes()
        .create_revision(
            "portable",
            &NewRecipeRevision::fake(vec![]),
            Some("portable-fake-v1"),
        )
        .unwrap();
    assert_eq!(revision.builder, "fake");
    let request = request.join().unwrap();
    assert!(request.starts_with("POST /api/v1/recipes/portable/revisions HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("idempotency-key: portable-fake-v1\r\n")
    );
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "builder": "fake",
            "definition": {},
            "required_capabilities": []
        })
    );
}

#[cfg(feature = "blocking")]
#[test]
fn build_target_creation_uses_typed_schedule_and_revision_identity() {
    let (url, request) = mock_once("201 Created", build_target_json());
    let client = stabbur_client::blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let revision = RecipeRevisionId::parse("01900000-0000-7000-8000-000000000010").unwrap();
    let target = client
        .build_targets()
        .create(
            "firefox-nightly",
            "firefox",
            revision,
            &std::collections::BTreeMap::new(),
            BuildTargetSchedule::Manual,
            None,
            true,
        )
        .unwrap();
    assert_eq!(target.name, "firefox-nightly");
    let request = request.join().unwrap();
    assert!(request.starts_with("POST /api/v1/build-targets HTTP/1.1\r\n"));
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["schedule"]["kind"], "manual");
    assert_eq!(body["recipe_revision"], revision.to_string());
}

#[cfg(feature = "blocking")]
#[test]
fn catalog_scan_request_is_pinned_and_idempotent() {
    let (url, request) = mock_once("201 Created", catalog_scan_json());
    let client = stabbur_client::blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let source = PinnedSource {
        url: "https://example.test/recipes.git".to_owned(),
        commit: "a".repeat(40),
    };
    let scan = client
        .catalog()
        .request_autopkg_scan(&source, "catalog-scan-a")
        .unwrap();
    assert_eq!(scan.producer, "autopkg");
    let request = request.join().unwrap();
    assert!(request.starts_with("POST /api/v1/recipe-catalog-scans HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("idempotency-key: catalog-scan-a\r\n")
    );
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["producer"], "autopkg");
    assert_eq!(body["source"]["revision"], "a".repeat(40));
}

#[cfg(feature = "blocking")]
#[test]
fn invalid_catalog_scan_source_is_rejected_before_transport() {
    let client = stabbur_client::blocking::Client::from_url("http://127.0.0.1:9")
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let error = client
        .catalog()
        .request_autopkg_scan(
            &PinnedSource {
                url: "https://user:secret@example.test/recipes.git".to_owned(),
                commit: "not-a-full-commit".to_owned(),
            },
            "catalog-scan-invalid",
        )
        .unwrap_err();
    assert!(matches!(
        error,
        stabbur_client::ApiError::InvalidValue { .. }
    ));
}

#[cfg(feature = "blocking")]
#[test]
fn invalid_fake_recipe_is_rejected_before_transport() {
    let client = stabbur_client::blocking::Client::from_url("http://127.0.0.1:9")
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let error = client
        .recipes()
        .create_revision(
            "portable",
            &NewRecipeRevision {
                builder: "fake".to_owned(),
                definition: serde_json::json!({"unexpected": true}),
                required_capabilities: vec![],
            },
            None,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        stabbur_client::ApiError::InvalidValue { .. }
    ));
}

#[cfg(feature = "blocking")]
#[test]
fn server_errors_and_debug_output_never_echo_bearer_tokens() {
    let body = r#"{"code":"forbidden","status":403,"detail":"Denied","request_id":"request-1","validation_errors":[]}"#;
    let (url, request) = mock_once("403 Forbidden", body);
    let token = "fixture-secret-token";
    let client = stabbur_client::blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(SecretToken::new(token).unwrap());
    let error = client.software().get("safe").unwrap_err();
    assert!(!format!("{error:?}").contains(token));
    assert!(!error.to_string().contains(token));
    request.join().unwrap();
}

#[cfg(feature = "blocking")]
#[test]
fn constrained_raw_requests_encode_paths_and_manage_sensitive_headers() {
    let (url, request) = mock_once("200 OK", r#"{"accepted":true}"#);
    let client = stabbur_client::blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let raw = RawRequest::new(RawMethod::Post, ["extensions", "../probe"])
        .unwrap()
        .query("selector", "a&b")
        .unwrap()
        .json(serde_json::json!({"requested": true}))
        .revision(4)
        .idempotency_key("raw-contract-test")
        .unwrap();
    let response = client.raw(&raw).unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(
        response.json::<serde_json::Value>().unwrap()["accepted"],
        true
    );

    let request = request.join().unwrap();
    assert!(request.starts_with("POST /api/v1/extensions/..%2Fprobe?selector=a%26b HTTP/1.1\r\n"));
    let lower = request.to_ascii_lowercase();
    assert!(lower.contains("authorization: bearer fixture-secret-token\r\n"));
    assert!(lower.contains("if-match: \"rev-4\"\r\n"));
    assert!(lower.contains("idempotency-key: raw-contract-test\r\n"));
    assert!(request.ends_with(r#"{"requested":true}"#));
}

#[cfg(feature = "blocking")]
#[test]
fn declared_oversized_json_is_rejected_before_buffering() {
    let (url, server) = mock_oversized_response();
    let client = stabbur_client::blocking::Client::from_url(&url)
        .unwrap()
        .authenticate(SecretToken::new("fixture-secret-token").unwrap());
    let error = client.software().list(None, 50).unwrap_err();
    assert!(matches!(error, stabbur_client::ApiError::ResponseTooLarge));
    server.join().unwrap();
}
