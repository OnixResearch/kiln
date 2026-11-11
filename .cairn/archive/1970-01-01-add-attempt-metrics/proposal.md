# Add opt-in attempt metrics

## Why

Issue OnixResearch/kiln#56 asks for a measure-first, default-off factory-health
receipt for claimed attempts. Missing cost data must be absent, not zero, and
metrics must not become an admission or landing condition.

## What Changes

- Add `AttemptMetrics` and typed absent/observed token and auxiliary-call
  observations.
- Record claim/end timestamps on attempts without changing claim, cancel,
  reservation, dispatch or verification decisions.
- Observe open attempts and record the first receipt only on explicit request.
- Add `kiln-implementer metrics` with optional usage input and explicit
  `--record true`.
- Document non-claims and add open, terminal, default-off, retained and invalid
  observation tests.

## Impact

- **Files**: `crates/kiln-core/src/implementation/metrics.rs`,
  `attempt.rs`, `kiln-implementer`, implementer CLI, docs and specification.
- **Testing**: absent usage, open/terminal elapsed, retained first receipt,
  stale fence, duplicate/oversized traces, pre-claim observation.
- **Authority**: observation only; no scoring, dashboard, incident model or
  admission/landing change.
