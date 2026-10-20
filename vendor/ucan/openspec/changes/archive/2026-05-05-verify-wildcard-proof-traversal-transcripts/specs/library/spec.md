## MODIFIED Requirements

### Requirement: Live linear proof traversal transcript checking [r[library.live-linear-proof-traversal-transcript]]

The UCAN shell MUST emit and verify an internal finite proof traversal transcript for compact-token verification paths whose proof graph is empty or exactly one parent per token and whose edges match the current primitive's resource-prefix and UCAN wildcard ability-containment model, using the vendored verified proof-traversal bridge before returning a verified token.

#### Scenario: Eligible linear chain is bridge checked [r[library.live-linear-proof-traversal-transcript.eligible-linear-chain-bridge-checked]]

- GIVEN a compact token verification path with no proof or a single-parent proof chain whose attenuation uses exact ability equality or verified wildcard ability containment and resource-prefix containment
- WHEN the shell has parsed, structurally validated, signature-verified, revocation-checked, and authorized the involved tokens
- THEN the shell MUST adapt the already available token facts into a parent-to-child transcript
- AND the shell MUST call the vendored proof traversal transcript bridge before returning success

#### Scenario: Multi-parent DAG remains explicit boundary [r[library.live-linear-proof-traversal-transcript.multi-parent-boundary]]

- GIVEN a token with multiple direct parent proofs
- WHEN the current linear transcript primitive cannot represent the proof graph exactly
- THEN the shell MUST leave that DAG on existing shell validation semantics
- AND documentation MUST state that a future verified DAG primitive is required before claiming bridge coverage for multi-parent proof graphs
