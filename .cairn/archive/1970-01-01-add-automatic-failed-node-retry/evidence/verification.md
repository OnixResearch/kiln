# Verification

Issue [#43](https://github.com/OnixResearch/kiln/issues/43).

`retry_failed` is a reviewed per-project option in the range 0–3, default 0.
When a run ends `DomainFailure`, `Denied`, `TimedOut` or `ProviderError`, `poll`
issues the existing failed-node rerun on the exact same source with the
deterministic request identity `auto-retry-<ordinal>-<parent run>`. The child
keeps the parent link and untrusted gate; the parent outcome never changes.

Disabled projects behave exactly as before. Unknown node states, unresolved
dirty snapshots, superseded untrusted runs, and request or run retention
refusals create no retry and leave the parent unchanged. The chain stops at the
reviewed bound.

The run detail and run list expose direct retry children. A failed parent with a
passing child is labelled "failed, then passed on retry"; the parent badge stays
failed. Because the retry child is a partial-node run, the existing required-CI
selector (`full_run && clean`) never treats it as a clean pass.

Evidence: `cargo test -p kiln-web-service --test engine` passed 95 tests,
including the passing-child, bound, disabled and profile-bound cases. Nickel
`typecheck` and the positive fixture export passed; the negative fixture with
`retry_failed = 4` was rejected. `node --check` passed for the UI asset.
