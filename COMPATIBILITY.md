# Server compatibility

The client, CLI, and server are independently versioned. Client 0.1.0 targets the released
Stabbur 0.1 API shape pinned in this repository. A published compatibility claim additionally
requires an immutable server image digest; a moving tag or local binary is not sufficient.

| Client | Server target | Immutable image            | Evidence                                                                                                                       | Status            |
| ------ | ------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ----------------- |
| 0.1.0  | 0.1.0         | Pending server publication | 75-operation reconciliation, mock HTTP boundary tests, async/blocking parity, and local four-repository workflow on 2026-09-05 | Release candidate |

The local live workflow covers authentication, software, builder-neutral recipes and a completed
deterministic target-triggered fake run/job, catalog scan request/cancellation,
principal/token administration, worker provision/rotation/drain,
streaming artifact ingestion and download, artifact HEAD and locations, stores, and audit. It does
not replace the final test against the exact image digest.

Catalog manifest planning and synchronization compose the same pinned software and recipe
operations. Catalog observations, scans, and build targets are typed parts of the pinned
75-operation contract.

Before publishing:

1. record the immutable `stabbur-server:0.1.0` image digest in `Cargo.toml` and this table;
2. run `scripts/run-integration-tests.sh` against that digest;
3. preserve the CI run URL or other immutable evidence; and
4. verify the pinned OpenAPI SHA-256 remains unchanged.
