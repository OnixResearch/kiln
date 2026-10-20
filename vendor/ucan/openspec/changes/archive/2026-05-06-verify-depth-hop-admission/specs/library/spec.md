## MODIFIED Requirements

### Requirement: Authorization uses verified capability semantics

Authorization matching and concrete request admission MUST reuse verified-logic-backed resource, ability, and proof-depth admission semantics used for proof-chain attenuation. Shell proof-hop depth admission MUST route the hop-to-chain-length conversion and overflow rejection through vendored verified-logic rather than local shell-only arithmetic.

#### Scenario: Proof-hop depth admission rejects overflow before budget check
- GIVEN a shell proof-hop depth that cannot be represented as `depth + 1` chain length
- WHEN proof-depth budget admission runs
- THEN the request MUST be rejected through vendored `verified-logic` proof-hop admission

#### Scenario: Proof-hop depth admission preserves existing budget semantics
- GIVEN a shell proof-hop depth and maximum delegation depth
- WHEN proof-depth budget admission runs
- THEN the result MUST match the existing chain-length budget predicate for `chain_len = depth + 1` when conversion succeeds
- AND the check MUST not claim proof traversal, backend, revocation, caveat, crypto, parser, clock, or provenance truth
