## MODIFIED Requirements

### Requirement: Revocation example onboards consumers [r[library.revocation.consumer-example]]
The repository MUST include a runnable example that demonstrates explicit caller-supplied revocation checks and fail-closed revocation backend errors without implying built-in registries, network fetches, persistence, replay stores, or credential-status formats.

#### Scenario: Example shows revocation backend errors fail closed [r[library.revocation.consumer-example.backend-error-fails-closed]]
- GIVEN a caller-supplied revocation checker that cannot answer for a token reference
- WHEN a consumer runs the revocation example
- THEN the example MUST show structured backend-error handling as a verification failure
- AND it MUST NOT treat an unavailable revocation backend as an allowed token
- AND it MUST NOT print compact UCAN token values
