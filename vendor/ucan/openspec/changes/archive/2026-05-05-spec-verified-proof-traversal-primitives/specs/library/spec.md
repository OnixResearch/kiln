## ADDED Requirements

### Requirement: Finite verified proof traversal primitives [r[library.verified-proof-traversal-primitives]]

The library MUST define future proof traversal verification in terms of finite, deterministic, data-only primitives that consume already-resolved structural inputs or shell-produced traversal transcripts, rather than resolver, proof-store, parser, cryptographic, storage, network, signing, persistence, replay, or ambient-clock side effects.

#### Scenario: Proof graph well-formedness is data-only [r[library.verified-proof-traversal-primitives.scenario.graph-well-formedness]]

- GIVEN finite parent and child proof entries with issuer, audience, proof reference, capability, temporal, and caveat-preservation fields
- WHEN the proof graph well-formedness primitive checks an edge
- THEN it MUST accept only edges whose child issuer matches the parent audience under verified proof-link semantics
- AND it MUST NOT fetch, parse, resolve, verify signatures, read storage, access a network, or consult an ambient clock

#### Scenario: Traversal budgets and cycles are checked over finite traces [r[library.verified-proof-traversal-primitives.scenario.bounded-traversal]]

- GIVEN a finite traversal trace with visited proof references, depth counters, proof counters, and input-byte counters
- WHEN the bounded traversal primitive checks the trace
- THEN it MUST reject cycles, duplicate visited references where they would create unsafe traversal, and exceeded depth, proof-count, or input-byte budgets
- AND it MUST preserve the shell requirement that limit violations fail closed

#### Scenario: Effective delegation folding preserves attenuation [r[library.verified-proof-traversal-primitives.scenario.effective-delegation-folding]]

- GIVEN an ordered finite proof chain from root through leaf
- WHEN the effective delegation folding primitive computes or checks the chain result
- THEN the result MUST preserve capability attenuation, temporal narrowing, and parent caveat payload preservation constraints at every step
- AND any over-broad resource, ability, time window, or missing preserved caveat MUST make the fold invalid

#### Scenario: Proof-chain authorization consumes folded results [r[library.verified-proof-traversal-primitives.scenario.chain-authorization]]

- GIVEN a finite proof chain, a folded effective delegation, and a requested invocation resource and ability
- WHEN the proof-chain authorization primitive decides whether the request is authorized
- THEN it MUST allow only requests contained by the effective delegated capability set and valid temporal window
- AND it MUST reject malformed, over-broad, expired, not-yet-valid, or disconnected chains

#### Scenario: Revocation-aware traversal denies from supplied facts [r[library.verified-proof-traversal-primitives.scenario.revocation-aware-chain]]

- GIVEN a finite proof chain and caller-supplied revocation facts for the leaf and proof-chain references
- WHEN the revocation-aware traversal primitive checks the chain
- THEN any supplied revoked fact MUST make the chain invalid
- AND the primitive MUST NOT query a revocation backend or convert unknown/backend-error states into allow decisions

#### Scenario: Traversal transcript consistency is checked canonically [r[library.verified-proof-traversal-primitives.scenario.transcript-consistency]]

- GIVEN a shell-produced traversal transcript containing requested proof references, resolved entries, parent-child links, fold steps, budget counters, revocation facts, and the final authorization target
- WHEN the transcript consistency primitive checks the transcript
- THEN it MUST reject transcripts whose visited references, edge order, counters, fold steps, revocation facts, or final decision are internally inconsistent
- AND a valid transcript MUST be sufficient for the UCAN shell to route matching traversal admission through verified logic without moving resolver, parser, proof-store, revocation-backend, crypto, or caveat-policy behavior into the primitive

## MODIFIED Requirements

### Requirement: Verified proof traversal model [r[library.verified-proof-traversal-model]]

The library MUST preserve the existing caller-injected proof traversal behavior while defining any future verified traversal bridge over finite resolved proof data and traversal transcripts rather than hidden resolver, proof-store, network, filesystem, cryptographic, revocation-backend, or ambient-clock side effects.

#### Scenario: Resolved traversal model preserves success semantics [r[library.verified-proof-traversal-model.scenario.success-semantics]]

- GIVEN a finite resolved proof chain whose references, parent-child issuer/audience links, temporal bounds, capability attenuation, caveat preservation, traversal budgets, and revocation facts are valid
- WHEN the shell routes matching traversal admission through verified traversal primitives
- THEN verification MUST still succeed for the same chain
- AND the resulting effective delegation and authorization decision MUST match the shell's existing semantics

#### Scenario: Backend boundaries remain shell owned [r[library.verified-proof-traversal-model.scenario.backend-boundaries]]

- GIVEN resolver, proof-store, revocation, parsing, cryptographic signature, compact-codec, or caller caveat-policy failures from shell dependencies
- WHEN proof traversal verification runs
- THEN those failures MUST remain fail-closed shell errors
- AND verified traversal primitives MUST NOT fetch proofs, resolve keys, verify signatures, access storage or network backends, persist state, replay-protect, sign, or read an ambient clock
