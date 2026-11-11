## Why

The web service can rerun only the failed nodes of a terminal run on the exact
same source, and the flaky view counts a test unstable only when the same
revision, pipeline, node, report and test have both passed and failed. Nothing
issued that rerun automatically, so the flaky view stayed empty unless an
operator retried by hand.

Issue: [#43](https://github.com/OnixResearch/kiln/issues/43).

## What Changes

- Add a reviewed per-project `retry_failed` option (0–3, default 0).
- When a run reaches a failed terminal outcome (`DomainFailure`, `Denied`,
  `TimedOut`, `ProviderError`), issue the existing `failed` rerun on the exact
  same source with the deterministic request identity
  `auto-retry-<ordinal>-<parent run>`.
- Mark the child with its automatic retry ordinal and keep the parent link.
- Keep every existing refusal: unknown node states, unresolved dirty snapshots,
  superseded untrusted runs, and request or run retention limits create no retry
  and leave the parent unchanged.
- Keep untrusted children untrusted at the exact approved commit.
- Show direct retry children in the run detail and run list, with a
  "failed, then passed on retry" badge on a failed parent whose child passed.

## Impact

- **Files**: `crates/kiln-web-service/src/{engine.rs,engine/queue.rs,
  engine/model.rs,engine/api.rs,engine/dashboard.rs,profile.rs}`,
  `crates/kiln-web-service/src/web.js`, `config/web-service.ncl`,
  `config/fixtures/{web-service-retry-positive,negative-web-service-retry-bound}.ncl`,
  `docs/operator.md`.
- **Testing**: passing-child and failed-parent projection, bound exhaustion,
  zero default, dispatch-limited backend, profile bound rejection, Nickel
  positive/negative fixtures.
- **Authority**: observation only. A partial-node retry is never a full clean
  run, so it cannot satisfy required CI; a passing child never rewrites the
  parent outcome or grants gate, merge or release authority.
