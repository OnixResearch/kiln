## ADDED Requirements

### Requirement: Verified backend transcript aggregation [r[library.verified-backend-transcript-aggregation]]
The library MUST route backend-operation transcript aggregation through vendored verified-logic predicates before accepting verification stages that depend on caller-injected key resolution, proof loading, revocation checking, or replay admission.

#### Scenario: Backend success transcript admits dependent stages [r[library.verified-backend-transcript-aggregation.scenario.success]]
- GIVEN key-resolution, proof-load, revocation-check, and replay-admission operation facts emitted by the shell around caller-injected hooks
- WHEN all required backend operation facts report success for the requested verification path
- THEN the shell MUST route the transcript through verified logic before accepting the dependent verification or invocation stage
- AND accepted backend transcript facts MUST be recorded without embedding backend payloads or secrets

#### Scenario: First backend failure wins [r[library.verified-backend-transcript-aggregation.scenario.first-failure]]
- GIVEN a backend transcript containing one or more missing, unsupported, malformed, stale, denied, duplicate, or backend-error outcomes
- WHEN the shell maps the transcript into structured verification or invocation errors
- THEN the first failed backend stage in documented order MUST determine the structured error class
- AND later success facts MUST NOT override an earlier failed backend stage

#### Scenario: Backend aggregation keeps backend truth outside the claim [r[library.verified-backend-transcript-aggregation.scenario.boundary-limits]]
- GIVEN backend transcript aggregation accepted by verified logic
- WHEN a consumer relies on actual key material, proof bytes, revocation registry state, nonce-store freshness, storage durability, network availability, or backend authorization semantics
- THEN the library MUST continue to document those facts as caller-owned runtime boundaries outside the backend transcript aggregation claim
