## MODIFIED Requirements

### Requirement: Recursive verification has deterministic limits
Proof-chain verification MUST enforce explicit limits for traversal depth, proof count, and serialized compact-token byte size. The token-size limit MUST apply to the input token and to each resolved proof token before parsing. Proof-depth budget admission MUST route through the vendored verified delegation-depth bridge so the shell does not maintain a parallel depth predicate.
r[library.resolver-proof-store.verification-limits]

#### Scenario: Proof-depth budget uses verified delegation-depth semantics [r[library.resolver-proof-store.verification-limits.depth-budget-uses-verified-logic]]

- GIVEN a verification traversal depth and a configured maximum proof depth
- WHEN the verifier decides whether recursion may continue
- THEN it MUST evaluate the depth budget through the verified delegation-depth bridge
- AND it MUST preserve the existing structured limit-exceeded error when the bridge rejects the depth
