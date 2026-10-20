## MODIFIED Requirements

### Requirement: Verification accepts explicit revocation checkers [r[library.revocation.explicit-checker]]

The library MUST allow callers to inject revocation decisions during token verification without relying on a process-global registry, and the in-memory `RevocationList` helper MUST route exact reference membership through the verified finite revocation-set bridge for both denial checks and duplicate insertion admission.

#### Scenario: RevocationList duplicate insertion uses verified set semantics [r[library.revocation.explicit-checker.dedupe-uses-verified-set]]

- GIVEN an in-memory `RevocationList` containing a proof reference
- WHEN the same proof reference is revoked again
- THEN the helper MUST consult the verified finite revocation-set bridge for membership
- AND the list MUST remain idempotent without adding a duplicate reference
