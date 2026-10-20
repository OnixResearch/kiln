## ADDED Requirements

### Requirement: Backend failure transcripts
The library SHALL expose and use a verified finite backend-operation transcript boundary for key resolution, proof loading, and revocation checks.

#### Scenario: Accepts only successful backend operations
- **WHEN** every operation fact in the transcript reports success
- **THEN** the backend transcript outcome is accepted

#### Scenario: Reports first failed backend operation
- **WHEN** one or more operation facts in the transcript report failure
- **THEN** the backend transcript outcome is rejected and identifies the first failed operation in transcript order

#### Scenario: Live verification fails closed on backend errors
- **WHEN** a caller-injected key resolver, proof store, or revocation checker returns a missing-data or backend error
- **THEN** verification returns the corresponding typed failure rather than authorizing the token
- **AND** the transcript boundary does not claim backend correctness, freshness, provenance, parsing, cryptography, hashing, I/O, or ambient runtime behavior
