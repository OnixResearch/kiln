## MODIFIED Requirements

### Requirement: Verification accepts explicit revocation checkers [r[library.revocation.explicit-checker]]
The library MUST keep revocation checks caller-injected and fail closed on backend errors. The deterministic in-memory revocation-list helper MUST perform finite proof-reference membership through the vendored verified revocation-set bridge rather than a shell-local membership scan.

#### Scenario: In-memory revocation list uses verified set semantics [r[library.revocation.verified-set-membership]]

- GIVEN a proof reference and an in-memory revocation list
- WHEN the revocation list evaluates membership
- THEN the lookup MUST pass through the verified revocation-set bridge helper
- AND listed references MUST deny verification while unlisted references remain allowed
