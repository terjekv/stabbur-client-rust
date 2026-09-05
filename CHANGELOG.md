# Changelog

## [Unreleased]

### Added

- Add validated catalog schema 2 with exact revision/append preconditions, source-pin proposals,
  software/queue summaries, worker draining and release withdrawal.
- Add bounded typed SSE decoding with reconnect/replay and hard run deadlines.

- Add builder-neutral `NewRecipeRevision` and async/blocking `create_revision`, including a checked
  deterministic fake-revision constructor.
- Add versioned catalog manifest models plus equivalent async/blocking deterministic `plan` and
  idempotent `sync` resources.
- Add async/blocking build-target list/get/create/update/trigger/run-history operations.
- Add immutable recipe-catalog snapshot list/get, exact lookup, and durable scan
  request/list/get/cancel operations with typed identities and models.

### Changed

- Reconcile healthz/readyz and the 75-operation contract; preserve login expiry and disable redirects.
- Keep async/blocking catalog orchestration shared and validate facts at typed boundaries.

- Wrap the typed AutoPkg helper in the builder-neutral public request contract and exercise a
  complete fake run/job in the independent live consumer.
- Make job ownership subject-neutral: build `run_id` and recipe-catalog scan identity are optional,
  mutually exclusive associations.

## [0.1.0] - 2026-08-26

### Added

- Complete typed coverage for the original 57 Stabbur 0.1 public operations.
- Async and blocking typestate clients with compile-time surface parity.
- Typed UUIDv7 identities, strict SHA-256 digests, cursor pages, lifecycle/catalog models, and
  redacted one-time credential types.
- Resource handles for identity, catalog, recipes, runs, artifacts, stores, workers, jobs, and
  audit.
- ETag mutation helpers, idempotency keys, run wait, live SSE, bounded/resumable downloads,
  blocking streaming uploads, and artifact HEAD helpers.
- A constrained authenticated `raw()` extension plus 2 MiB request and 8 MiB buffered-response
  limits without exposing HTTP implementation types.
- Reviewed OpenAPI reconciliation, loopback mock HTTP tests, and an independent full-workflow
  consumer.
- Pin the adapter-owned `/stabbur/recipe_trust_succeeded` selector contract used by AutoPkg
  revisions and the independent consumer.
