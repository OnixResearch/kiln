## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST verify finite proof-store result transcripts so proof resolution either records a successful returned proof for the requested reference or fails closed on backend/missing-proof errors before traversal consumes proof material.

#### Scenario: Proof-store results use verified transcript bridge [r[library.compact-token.scenario.proof-store-result-transcript-admission]]

- GIVEN a caller-injected proof store and a proof-reference lookup during verification
- WHEN the shell receives a success, missing-proof, or backend-error result
- THEN it MUST route requested-reference, result-kind, returned-proof-presence, and first-failure facts through a vendored verified proof-store result transcript bridge
- AND backend or missing-proof failures MUST fail closed before authorization

#### Scenario: Proof-store result transcripts keep backend truth outside the verified claim [r[library.compact-token.scenario.proof-store-result-transcript-boundary-limits]]

- GIVEN proof-store result transcript facts admitted by the verified predicate
- WHEN the shell parses, verifies, or authorizes returned proof material
- THEN the library MUST continue to document proof-store honesty, freshness, availability, storage/network behavior, returned compact-token truth, proof provenance, parser correctness, and cryptography as outside the transcript-admission claim
