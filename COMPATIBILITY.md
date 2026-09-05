# Server compatibility

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
