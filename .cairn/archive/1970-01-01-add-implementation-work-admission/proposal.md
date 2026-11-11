## Why

Kiln can represent issues and run exact candidate CI, but logged intent is not
yet an admitted implementation job. An issue or linked Cairn task needs its own
identity, version, readiness, and authorization boundary before a coding worker
runs.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add a bounded, versioned `ImplementationWork` contract in `kiln-core` binding
  identity, source aliases and versions, enrollment, requested outcome,
  acceptance obligations, dependencies, permitted paths and effects,
  verification profile, budget, terminal policy, and priority.
- Add canonical identity and alias-key derivation over a length-prefixed field
  encoding.
- Add a pure `WorkLedger` with duplicate, stale, ambiguous, superseded, blocked
  and ready decisions, explicit task-level source mapping, dependency-cycle
  detection, and deterministic ranking.
- Refuse unknown schemas, unenrolled repositories/actors, unsafe paths, unknown
  effects, missing acceptance or budget, and oversized inputs before retention.
- Document the contract and the still-unwired GitHub/Cairn adapters.

## Impact

- **Files**: `crates/kiln-core/src/implementation.rs`,
  `crates/kiln-core/src/lib.rs`, `docs/implementation-work.md`.
- **Testing**: duplicate/out-of-order, explicit mapping and forgery, missing
  dependency and cycle, supersession and replay, refusal, and lifecycle-state
  boundary tests.
- **Authority**: admission only. No claim, execution, verification, publication,
  or completion authority is added. The GitHub issue webhook adapter, Cairn #1
  snapshot consumer, and Mneme qualification remain external work.
