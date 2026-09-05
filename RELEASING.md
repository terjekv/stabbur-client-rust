# Releasing

1. Release `stabbur-server` and record its immutable image digest.
2. Pin the server version, image, complete OpenAPI document, and normalized snapshot here.
3. Reconcile generated code and implement every contract change or record an intentional gap.
4. Run formatting, Clippy, rustdoc, tests, all feature combinations, MSRV, supply-chain, SemVer,
   pinned integration, and independent `e2e_client` checks.
5. Update `COMPATIBILITY.md` and the dated changelog section.
6. Package from a clean checkout. Tag `vX.Y.Z` only after the same commit passes main CI.

Only `stabbur_client` is publishable. The reconciliation tool and e2e consumer remain private.
