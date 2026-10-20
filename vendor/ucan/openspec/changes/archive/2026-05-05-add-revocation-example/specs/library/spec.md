## ADDED Requirements

### Requirement: Revocation example onboards consumers [r[library.revocation.consumer-example]]
The repository MUST include a runnable example that demonstrates explicit caller-supplied revocation checks without implying built-in registries, network fetches, persistence, replay stores, or credential-status formats.

#### Scenario: Example shows proof-chain revocation denial [r[library.revocation.consumer-example.parent-proof-denied]]
- GIVEN a child token with a valid parent proof and a caller-supplied in-memory revocation list containing the parent proof reference
- WHEN a consumer runs the revocation example
- THEN the example MUST first show successful verification with no revocations
- AND it MUST then show structured revocation denial for the revoked parent proof reference
- AND it MUST NOT print compact UCAN token values
