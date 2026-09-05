# Security policy

Report vulnerabilities privately to the maintainers. Never place tokens, passwords, worker
credentials, protected URLs, request bodies containing secrets, or a working exploit in public
issues, fixtures, command transcripts, or logs.

## Client security properties

- Base URLs must be absolute HTTP(S) origins without credentials, path prefixes, query strings, or
  fragments.
- Every dynamic path value is percent-encoded as one opaque segment; traversal, query, and fragment
  characters cannot change the endpoint.
- Bearer credentials are sent only in the authorization header.
- Token, password, and one-time worker/API credential types redact `Debug`; transport errors omit
  URLs and headers.
- UUID identities validate UUIDv7 and artifact identities validate lowercase SHA-256 at the public
  boundary.
- Page sizes are limited to 1 through 200.
- Blocking downloads and uploads use bounded streaming; async downloads return a bounded stream.
- Buffered JSON and constrained-extension responses are rejected above 8 MiB, including a declared
  oversized body before allocation.
- Resume requests fail closed if the server does not return `206 Partial Content`.
- Mutable helpers always require an explicit revision and send a strong `If-Match` value.
- The low-level worker execution protocol is not part of this public crate.
- The constrained authenticated `raw()` extension percent-encodes every segment, cannot address
  `/api/v1/internal`, does not accept arbitrary headers, and redacts its body from `Debug`.

Applications still own protected credential storage, TLS trust policy, output-file permissions,
artifact digest verification after download, retry policy, and safe handling of provenance or
audit data. Prefer HTTPS for every non-loopback connection.
