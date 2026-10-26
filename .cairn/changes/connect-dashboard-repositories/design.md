# Design: real repository dashboard integration

## Context

The existing dashboard validates event identities and canonical replay, then uses the accepted observation projection. Its one-input CLI must remain usable. Real Radicle histories are candidate-local, while GitHub stores event vectors inside profile-bound run records.

## Decisions

- Keep provider code out of the dashboard's pure projection. A GitHub-shell exporter reads an existing profile-bound database without creating schema or changing WAL settings, validates the selected run, frames its original events as canonical records, and replays them before emission. The exporter neither resolves secrets nor publishes statuses.
- Add a catalog input containing bounded project declarations and bounded explicit history inputs. Each history identifies a journal or canonical store stream, with optional declared source/revision labels and selection. Labels are operator declarations, not newly verified source facts. Resolve relative paths against the catalog location, not ambient working directory. Reject duplicate identities, unknown fields, malformed paths/streams and unsupported versions before rendering.
- Independently admit every selected catalog input. Do not concatenate journals, reinterpret their revisions, or claim a global observation identity. Render one bounded self-contained HTML artifact with accessible repository/history navigation; keep canonical identities and non-claims per observation. Preserve selection across watch reloads and expose all admitted history without JavaScript. Report removed selections rather than silently selecting another history.
- Keep transfer outside the dashboard. A separately configured imperative publisher invokes the real dashboard, retains its last good HTML on generation or transfer failure, and atomically installs only that HTML at the operator-declared Aspen3 destination. Use bounded processes, fixed argv, explicit paths and no automatic execution/status retries.
- Use existing live service configuration and real retained self-CI journals where available. A fresh local read-only source run may exercise source-to-CI without Radicle signing. Remote mutation and publication are not inferred from a request to connect observation.

## Risks and Mitigations

- Aggregate output/memory: impose named catalog, project/history, input and rendered-byte bounds.
- Cross-history identity collisions: isolate reports, preserve source journal boundaries, and key navigation by project/history rather than run ID alone.
- Untrusted labels/events: escape data and keep trusted scripts fixed; no browser provider/network access.
- Stale display: watched output identifies observed revisions, retains no newest/completeness claim, and leaves the last good report intact on failure.
- Local/remote path confusion: resolve catalog inputs deterministically and validate explicit publication targets before effects.

## Verification

Exercise valid and invalid catalog/export inputs, exact replay outcomes, missing/removed selection, no-script accessibility, watch navigation, failing generation, interrupted transfer, duplicate events and restart behavior. Record fresh versus retained evidence, source revisions, actual process results, and authority limits. Run focused package checks and required repository gates after implementation.
