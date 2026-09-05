# Stabbur Rust client

`stabbur_client` is the supported Rust API for Stabbur 0.1. It is independently versioned from
the server, uses typestate to separate unauthenticated and authenticated operations, and exposes
equivalent asynchronous and blocking resource handles. The asynchronous client is enabled by
default; synchronous applications select the `blocking` feature.

## Quick start

```rust,no_run
use stabbur_client::{Client, Credentials};

# async fn example() -> Result<(), stabbur_client::ApiError> {
let public = Client::from_url("https://stabbur.example.net")?;
let client = public
    .login(&Credentials::new("operator", "password-from-a-secret-source"))
    .await?;

let page = client.software().list(None, 50).await?;
for software in page.items {
    println!("{} ({})", software.name, software.slug);
}
# Ok(())
# }
```

For a service token, load the value from a protected file or secret provider and attach it without
performing login:

```rust,no_run
use stabbur_client::{Client, SecretToken};

# fn example(token_from_protected_storage: String) -> Result<(), stabbur_client::ApiError> {
let client = Client::from_url("https://stabbur.example.net")?
    .authenticate(SecretToken::new(token_from_protected_storage)?);
# let _ = client;
# Ok(())
# }
```

`SecretToken`, `Credentials`, created API tokens, and worker credentials redact their secret
material from `Debug` and errors. Do not pass passwords or bearer tokens as process arguments.

## v0.1 resource surface

The pinned contract contains 75 public operations. Reviewed specifications reconcile every
operation to a client implementation at build time.

| Handle            | Operations                                                                 |
| ----------------- | -------------------------------------------------------------------------- |
| `identity()`      | principals, password changes/resets, session revocation, API tokens, roles |
| `catalog()`       | desired-state plan/sync, snapshots, exact lookup, durable scan lifecycle   |
| `build_targets()` | manual/interval desired policy, updates, triggers, and run history         |
| `software()`      | software, releases, variants, channels, promotion, rejection, resolution   |
| `recipes()`       | recipe metadata, validated builder revisions, recipe runs                  |
| `runs()`          | create, inspect, cancel, wait, ordered logs, live SSE events               |
| `artifacts()`     | metadata, locations, verified upload, HEAD, range/resumable download       |
| `stores()`        | configured stores and non-mutating adapter probes                          |
| `workers()`       | provision, inspect, capability ceilings, drain/enable, credential rotation |
| `jobs()`          | subject-neutral job pages and complete durable job details                 |
| `audit()`         | cursor-paginated append-only events                                        |

Mutable operations accept the current numeric `revision`; the client emits the strong
`If-Match: "rev-N"` header. Passing revision `0` creates a channel. Retryable create/cancel methods
accept explicit idempotency keys so callers can safely retry after transport failures.

`CatalogManifest::canonicalized` validates schema version 1, rejects duplicate identities, sorts
entries deterministically, and normalizes builder-mandatory capabilities. Both client modes expose
`catalog().plan(...)` for a read-only ordered diff and `catalog().sync(...)` for additive,
history-preserving reconciliation. Unlisted resources are untouched and no build is scheduled.

`catalog()` also exposes immutable worker observations and durable server-requested scans of exact
pinned AutoPkg repositories. Discovery remains separate from desired execution:
`build_targets()` binds reviewed immutable recipe revisions to manual or fixed-interval policy.

## Streaming and long-running work

- Async artifact downloads return a crate-owned bounded byte stream.
- Blocking downloads copy through a fixed 64 KiB buffer and support resume offsets.
- Blocking file uploads accept a reader plus an exact length and never buffer the full file.
- Buffered JSON responses are rejected above 8 MiB, including malformed error responses.
- `runs().events(...)` exposes replay followed by live SSE bytes without exposing a `reqwest`
  response type.
- `runs().wait(...)` polls until succeeded, failed, or cancelled with caller-selected deadlines.

The public client administers workers but deliberately excludes the claim, heartbeat, log-submit,
artifact-attempt, and completion protocol. That protocol belongs to the server-supplied worker
runtime. See the server's worker operations guide for provisioning and AutoPkg execution.

For a public operation introduced by a compatible server before a typed client release,
authenticated clients provide `raw(&RawRequest)`. The checked request encodes opaque segments,
cannot address `/api/v1/internal`, manages authentication itself, restricts headers to revision and
idempotency controls, caps JSON requests at 2 MiB, and caps responses at 8 MiB. It does not expose
`reqwest` types. Typed resource methods remain the stable interface.

## Contract maintenance

- `openapi/openapi.json` is the complete server 0.1 document.
- `openapi/operations.json` is the normalized operation/security/content snapshot.
- `stabbur_reconcile/specs/operations.json` is the reviewed coverage specification.
- `src/resources/generated.rs` is committed generated source.

Downstream builds run no generator. Update the pinned document, normalize it, review every changed
operation, run the reconciler, and then update models and both client modes. The unpublished
`e2e_client` is a genuinely independent consumer and runs a full control-plane workflow against a
server before compatibility is claimed.

Transport-boundary tests inject a one-request loopback HTTP implementation through `from_url` and
assert the exact method, encoded target, headers, body, bounded-response behavior, redaction, and
error mapping without a live server. This exercises the same private transport used by resource
methods while keeping third-party HTTP types out of the public API.

## Verification

Rust 1.88 is the MSRV. The exact local commands are in [AGENTS.md](AGENTS.md); they cover formatting,
strict Clippy, rustdoc, tests, all four feature combinations, MSRV, OpenAPI reconciliation,
packaging, and dependency/license policy. [COMPATIBILITY.md](COMPATIBILITY.md) distinguishes local
release-candidate evidence from the immutable-image evidence required to publish.

## Coordinated operator workflows

The current unreleased contract includes catalog schema 2, exact target/revision reconciliation,
software status, release withdrawal, worker draining, and bounded reconnecting run watches.
See the server's [operator workflow guide](../stabbur/docs/operator-workflows.md) and the independent
[management console](../stabbur-frontend/README.md). Schema 1 catalogs remain accepted without targets.
Local cross-repository integration does not replace immutable released-image acceptance.
