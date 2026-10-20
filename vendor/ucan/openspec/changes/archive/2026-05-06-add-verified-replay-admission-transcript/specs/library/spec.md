## ADDED Requirements

### Requirement: Verified replay admission transcript [r[library.verified-replay-admission-transcript]]
The library MUST route replay-admission outcome aggregation through a vendored verified-logic predicate before an invocation with replay admission is accepted or denied for replay reasons.

#### Scenario: Replay admission accepts only an allowed transcript [r[library.verified-replay-admission-transcript.scenario.accepts-allowed]]
- GIVEN an invocation that has already passed compact-token verification and authorization
- WHEN the caller-supplied replay admission hook reports an allowed replay outcome
- THEN the shell MUST admit that outcome through a verified replay-admission transcript predicate
- AND the invocation MAY be accepted only after that verified transcript accepts

#### Scenario: Replay admission fails closed by outcome class [r[library.verified-replay-admission-transcript.scenario.fail-closed-classes]]
- GIVEN a replay admission hook that reports duplicate, stale, malformed, denied, or backend failure
- WHEN `verify_invocation_with_replay` decides the invocation result
- THEN the shell MUST route the reported outcome class through the verified replay-admission transcript predicate
- AND the invocation MUST be denied without treating any replay failure as authorization success

#### Scenario: Replay transcript keeps backend truth outside the claim [r[library.verified-replay-admission-transcript.scenario.boundary-limits]]
- GIVEN a replay-admission transcript admitted by verified logic
- WHEN a consumer relies on nonce uniqueness, request identity, wall-clock freshness, crash recovery, or durable storage behavior
- THEN the library MUST continue to document those facts as caller-owned runtime boundaries outside the verified replay transcript claim
