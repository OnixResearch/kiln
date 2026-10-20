## ADDED Requirements

### Requirement: Live linear proof traversal transcript checking [r[library.live-linear-proof-traversal-transcript]]

The UCAN shell MUST emit and verify an internal finite proof traversal transcript for compact-token verification paths whose proof graph is empty or exactly one parent per token and whose edges match the current primitive's exact ability/resource-prefix model, using the vendored verified proof-traversal bridge before returning a verified token.

#### Scenario: Eligible linear chain is bridge checked [r[library.live-linear-proof-traversal-transcript.eligible-linear-chain-bridge-checked]]

- GIVEN a compact token verification path with no proof or a single-parent proof chain whose attenuation uses exact ability equality and resource-prefix containment
- WHEN the shell has parsed, structurally validated, signature-verified, revocation-checked, and authorized the involved tokens
- THEN the shell MUST adapt the already available token facts into a parent-to-child transcript
- AND the shell MUST call the vendored proof traversal transcript bridge before returning success

#### Scenario: Bridge rejection fails closed [r[library.live-linear-proof-traversal-transcript.bridge-rejection-fails-closed]]

- GIVEN an eligible linear proof chain whose shell-produced transcript is rejected by the verified bridge
- WHEN verification reaches transcript checking
- THEN verification MUST fail with a proof-chain error
- AND the token MUST NOT be returned as verified

#### Scenario: Multi-parent DAG remains explicit boundary [r[library.live-linear-proof-traversal-transcript.multi-parent-boundary]]

- GIVEN a token with multiple direct parent proofs
- WHEN the current linear transcript primitive cannot represent the proof graph exactly
- THEN the shell MUST leave that DAG on existing shell validation semantics
- AND documentation MUST state that a future verified DAG primitive is required before claiming bridge coverage for multi-parent proof graphs

#### Scenario: Shell boundaries are preserved [r[library.live-linear-proof-traversal-transcript.boundary-preserved]]

- GIVEN live transcript checking is enabled for an eligible chain
- WHEN the shell builds the transcript
- THEN the builder MUST NOT fetch proofs, parse compact tokens, verify signatures, query revocation backends, evaluate arbitrary caveat policies, persist state, access the network or filesystem, sign data, or read an ambient clock
- AND those effects MUST remain in the existing caller-injected shell boundaries
