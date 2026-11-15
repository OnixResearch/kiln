## Why

Issue #65: reviewers can launch an exact admitted artifact, but cannot inspect a
selected baseline and the current candidate together with their preview
observations and identities. A successful guest exit or similar output can
otherwise be attributed to the wrong revision or mistaken for canonical
verification. This change adds the first, CLI-only slice; the graphical cohort
in the issue comment depends on #66, #67 and #68 and stays out of scope.

## What Changes

- The disposable preview server accepts one bounded browser report per launch
  from its own origin and retains a server-composed observation record: the
  admitted descriptor identity, CI binding, engine, platform, invocation,
  phases, bounded result, server observation time, finite expiry and
  provenance. Its identity is a domain-tagged BLAKE3 of the record.
  Engine-only qualification cannot retain review observations.
- The review service reads an explicitly configured, operator-owned
  observation directory read-only and projects only records for the change's
  repository, project and revision commits. A record is current only for the
  current revision commit and the selected successful CI run; everything else
  is historical.
- A read-only comparison route pairs one projected baseline with one current
  candidate and returns a deterministic comparison identity plus explicit
  incompatibility reasons.
- The review page shows both lanes side by side with exact identities, phases
  and bounded output, restores the pair from a deep link and never offers a
  launch for the baseline lane.

## Impact

Consumer: authenticated change reviewers. Owner: Kiln review/web and preview
maintainers. Files: preview observation module, preview server and shell, the
artifact-preview example, review projection and routes, review UI, tests, docs
and changelog. Preview observations stay preview evidence: they never write CI,
review approval, merge or implementation-completion state.

## Out of Scope

Graphical captures, historical-baseline launch, rebuild on miss, new browser
engines, new review authority and comparison of unlike invocations as if only
source changed.
