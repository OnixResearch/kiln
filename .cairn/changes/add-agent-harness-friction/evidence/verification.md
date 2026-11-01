# Verification evidence

## Result

Review-friction observation now carries and aggregates declared agent harnesses.
Each change contributes once per declared harness; agents without harness
metadata are not assigned one. Empty maps preserve earlier identities.

## Evidence

- Observation tests cover window projection, aggregation and identity changes.
- Dashboard tests cover plain and HTML rendering.
- Selected core, observation, dashboard and web-service suites pass.
- Workspace check, workspace-wide Clippy, rustfmt and Cairn validation pass.

## Non-claims

Declared harnesses are not verified execution. Historical windows are not
backfilled, and harness counts grant no review, CI, merge or release authority.
