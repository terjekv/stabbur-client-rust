# Server compatibility

Client 0.0.1 targets released Stabbur server 0.0.1 and the complete pinned 75-operation contract.

| Client | Server | Platform    | Evidence                                                                     | Status         |
| ------ | ------ | ----------- | ---------------------------------------------------------------------------- | -------------- |
| 0.0.1  | 0.0.1  | Linux amd64 | [Exact sources, CI runs, image and OpenAPI hash](evidence/server-0.0.1.json) | Image verified |

The [server publication run](https://github.com/terjekv/stabbur/actions/runs/36231611736) passed the client,
independent downstream consumer, CLI and browser suite against this immutable image:

```text
ghcr.io/terjekv/stabbur-server@sha256:41aba0e2051de7d74df09a6e57487a4e1755706d587aca54603167953fd0748d
```

The [disposable macOS acceptance](https://github.com/terjekv/stabbur/actions/runs/36231299162) separately passed
AutoPkg catalog discovery/import, build and publication, Munki installation/detection, browser
workflows, backup restoration, crash recovery, lease fencing and withdrawal.

The client release workflow repeats the live suite and full OpenAPI comparison for the exact
release source after its main CI passes, verifies `cargo package`, and attaches
`release-evidence.json` to the [0.0.1 release](https://github.com/terjekv/stabbur-client-rust/releases/tag/v0.0.1).
The package supports Rust 1.88 and async/blocking feature combinations. This is the initial
supported client release; later public API changes follow the documented compatibility policy.

Repeat the live check with Docker available:

```bash
STABBUR_INTEGRATION_SERVER_IMAGE='ghcr.io/terjekv/stabbur-server@sha256:41aba0e2051de7d74df09a6e57487a4e1755706d587aca54603167953fd0748d' \
  scripts/run-integration-tests.sh
```

Historical evidence below remains tied to its original development sources.

## Previous development-image acceptance

The client, CLI, and server are independently versioned. Client 0.1.0 targets the Stabbur 0.1
API shape pinned in this repository. Compatibility evidence identifies exact source commits
and an immutable image digest; it does not declare a crates.io or tagged source release.

| Client | Server target | Platform    | Evidence                                                         | Status         |
| ------ | ------------- | ----------- | ---------------------------------------------------------------- | -------------- |
| 0.1.0  | 0.1.0         | Linux amd64 | [Recorded source and image evidence](evidence/server-0.1.0.json) | Image verified |

The [publication workflow](https://github.com/terjekv/stabbur/actions/runs/33989125573) ran the independent consumer and client
suite against this exact published image:

```text
ghcr.io/terjekv/stabbur-server@sha256:19a0c843a1622173f8efd3012ddd223a93c8f487a6e5c003b3de2f3387d7f723
```

The live consumer covers authentication, software, builder-neutral recipes and a completed
deterministic target-triggered fake run/job, catalog scan request/cancellation,
principal/token administration, worker provision/rotation/drain, streaming artifact ingestion
and download, artifact HEAD and locations, stores, and audit. Mock tests additionally verify
transport bounds, redaction, and async/blocking behavior. The pinned contract has 75 operations.

The [macOS acceptance run](https://github.com/terjekv/stabbur/actions/runs/33988455119) separately proves source-built browser
management, AutoPkg delivery, actual Munki installation/detection, backup restoration, crash
recovery and withdrawal. It does not claim macOS-container or separate-host network coverage.

To repeat the image check with Docker available:

```bash
STABBUR_INTEGRATION_SERVER_IMAGE='ghcr.io/terjekv/stabbur-server@sha256:19a0c843a1622173f8efd3012ddd223a93c8f487a6e5c003b3de2f3387d7f723' \
  scripts/run-integration-tests.sh
```

For another server image, rerun the live suite before updating the manifest, workflow defaults,
this matrix and the recorded evidence. Preserve the OpenAPI hash and exact tested source commits.
