## ADDED Requirements

### Requirement: Verified proof traversal model [r[library.verified-proof-traversal-model]]

The library MUST preserve the existing caller-injected proof traversal behavior while defining any future verified traversal bridge over finite resolved proof data rather than hidden resolver, proof-store, network, filesystem, or cryptographic side effects.

#### Scenario: Resolved traversal model preserves success semantics [r[library.verified-proof-traversal-model.scenario.success-semantics]]

- GIVEN a finite resolved proof chain whose references, parent-child issuer/audience links, temporal bounds, capability attenuation, and traversal budgets are valid
- WHEN the shell routes matching traversal admission through a verified traversal bridge
- THEN verification MUST still succeed for the same chain
- AND the resulting effective delegation MUST match the shell's existing semantics

#### Scenario: Resolved traversal model rejects unsafe traversal states [r[library.verified-proof-traversal-model.scenario.rejects-unsafe-states]]

- GIVEN a finite resolved proof chain with a cycle, broken link, exceeded depth/count/input budget, malformed proof reference, or over-broad delegation
- WHEN traversal admission is checked through a verified traversal model or documented shell fallback
- THEN verification MUST fail with a structured traversal, proof-chain, limit, or attenuation error
- AND the token MUST NOT be treated as authorized

#### Scenario: Backend boundaries remain shell owned [r[library.verified-proof-traversal-model.scenario.backend-boundaries]]

- GIVEN resolver, proof-store, revocation, parsing, or cryptographic signature failures from caller-injected shell dependencies
- WHEN proof traversal verification runs
- THEN those failures MUST remain fail-closed shell errors
- AND the verified traversal model MUST NOT fetch proofs, resolve keys, verify signatures, access storage or network backends, persist state, replay-protect, or read an ambient clock
