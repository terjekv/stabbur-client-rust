# Stabbur Rust client contributor guidance

This workspace contains the supported `stabbur_client` crate, unpublished `stabbur_reconcile`
contract tool, and unpublished `e2e_client` downstream consumer. The server workspace is not a
public Rust API.

## Sources of truth

- Root `Cargo.toml` owns the client version, MSRV, features, target server release, and immutable
  target image.
- `openapi/openapi.json` is the complete pinned server document. `openapi/operations.json` is its
  normalized operations/security/content snapshot.
- Reviewed resource specifications live in `stabbur_reconcile/specs`; generated Rust under
  `src/resources/generated.rs` is committed and must not be edited by hand.
- `COMPATIBILITY.md` defines release evidence. A floating server branch never changes a published
  compatibility claim.

Keep all of those sources, `TARGET_SERVER_VERSION`, the workflows, README, changelog, and
compatibility matrix synchronized.

## Verification

```text
cargo fmt --all -- --check
cargo run -p stabbur_reconcile --locked -- check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo test --workspace --all-features --locked
cargo check -p stabbur_client --locked --no-default-features
cargo check -p stabbur_client --locked --no-default-features --features async
cargo check -p stabbur_client --locked --no-default-features --features blocking
cargo check -p stabbur_client --locked --no-default-features --features async,blocking
cargo +1.88 check --workspace --all-targets --all-features --locked
python3 scripts/openapi-contract.py validate
cargo deny check
```

Run the pinned live suite and independent consumer before making a compatibility claim. Until a
released server image digest replaces `pending-server-release`, live evidence is intentionally not
claimable.

## Architecture

- Preserve typestate: unauthenticated clients expose login and token attachment; authenticated
  clients expose administrative resources.
- Keep async and blocking behavior equivalent and extend compile-time parity checks with every
  public operation.
- Centralize endpoint paths. Encode every dynamic value as one opaque path segment and prevent
  base-URL origin, traversal, query, fragment, and authorization-header escape.
- Keep transport-independent models, pagination, problem parsing, redaction, and validation shared.
- Expose crate-owned download streams and results rather than `reqwest` types.
- Prefer typed IDs, validated digests, resource handles, and checked builders over primitive or
  positional APIs.
- Keep token, password, authorization, and credential values redacted from `Debug`, errors, URLs,
  and logs.
- The public client exposes administrative worker resources, never internal claim, heartbeat, or
  result protocol primitives.

Every contract change needs endpoint/model updates, async/blocking parity, mock behavior tests,
normalized OpenAPI coverage, and pinned-server evidence when available. Review the changelog for
every pull request and call out breaking changes with migration guidance.

## Validated contracts

- Preserve validated facts as types across architectural boundaries. Convert raw API, file,
  command-line, and persistence representations once with a fallible constructor. Keep the
  resulting proof type private-fielded and pass it to downstream operations instead of rebuilding
  or rechecking the fact. Deserialization must use that constructor.
- Use newtypes for scalar invariants, enums for mutually exclusive or correlated states, and
  capability/proof wrappers for validated, resolved, authenticated, or planned state. Keep raw
  transport DTOs distinct from validated application values; do not expose mutable proof fields.
- Types complement database constraints, transactions, optimistic concurrency, and lease fencing.
  A validated snapshot cannot prove that mutable remote state remains current. Preserve revision
  and idempotency preconditions across calls and report stale state explicitly.
