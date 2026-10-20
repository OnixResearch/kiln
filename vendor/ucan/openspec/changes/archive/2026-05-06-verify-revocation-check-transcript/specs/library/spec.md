## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST verify finite revocation-check transcripts so each queried reference records either a successful revoked/not-revoked fact or a backend failure that denies authorization before the token or proof is accepted.

#### Scenario: Revocation checks use verified transcript bridge [r[library.compact-token.scenario.revocation-check-transcript-admission]]

- GIVEN a caller-injected revocation checker and proof/token references encountered during verification
- WHEN the shell receives revoked, not-revoked, or backend-error results
- THEN it MUST route checked-reference, result-kind, and first-failure facts through a vendored verified revocation-check transcript bridge
- AND revoked or backend-error results MUST fail closed before authorization

#### Scenario: Revocation transcripts keep registry truth outside the verified claim [r[library.compact-token.scenario.revocation-check-transcript-boundary-limits]]

- GIVEN revocation-check transcript facts admitted by the verified predicate
- WHEN the shell treats a token or proof as non-revoked
- THEN the library MUST continue to document revocation registry truth, freshness, external authority correctness, backend availability, network/storage behavior, reference provenance, and cryptography as outside the transcript-admission claim
