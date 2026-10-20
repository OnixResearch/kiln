## MODIFIED Requirements

### Requirement: Delegation proof chains are verified end to end
The library MUST verify delegation proof chains across parent tokens, including issuer/audience linkage, temporal narrowing, and capability attenuation. Issuer/audience proof-link admission MUST route through the vendored verified-chain bridge rather than a shell-local direct semantic comparison.

#### Scenario: Proof link uses verified chain semantics [r[library.delegation-chain.proof-link-uses-verified-chain-semantics]]

- GIVEN a parent token audience DID and a child token issuer DID
- WHEN the shell verifies proof-chain linkage
- THEN the parent-audience/child-issuer check MUST pass through the verified-chain bridge helper
- AND a mismatch MUST still fail with a proof-chain error
