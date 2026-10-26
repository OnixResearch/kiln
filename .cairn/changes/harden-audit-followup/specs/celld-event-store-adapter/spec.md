## MODIFIED Requirements

### Requirement: Authenticated bounded transport

r[kiln.celld.transport] The adapter MUST use one admitted Worker origin, path, TLS policy, authentication profile, and named request and response bounds.

The initial contract MUST use HMAC-SHA256 request authentication with shell-owned secret references. The deployed Worker MUST use platform MAC verification, reject timestamps outside its named five-minute skew window before object resolution, and retain mutation nonce admission atomically with the corresponding SQL operation. Redirects and request-supplied destinations MUST remain disabled. These transport decisions MUST NOT be inferred from the in-memory Rust store model.

#### Scenario: Authenticated request is valid

r[kiln.celld.transport.scenario.valid]
- GIVEN a bounded request with an admitted key, timestamp, nonce, body digest, and signature
- WHEN the Worker verifies the envelope
- THEN it MAY resolve the admitted partition and process the operation.

#### Scenario: Destination or signature is invalid

r[kiln.celld.transport.scenario.rejected]
- GIVEN a request changes the configured origin, follows a redirect, replays a mutation nonce, presents an out-of-window timestamp, or has an invalid signature
- WHEN transport or Worker admission runs
- THEN it MUST reject before a Durable Object mutation.
