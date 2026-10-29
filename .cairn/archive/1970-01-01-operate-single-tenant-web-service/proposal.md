## Why

Kiln's current dashboard is an offline read-only artifact. Its current consumer,
Kiln self-CI, needs a single-tenant service that joins existing source, workflow,
canonical-state, logs and product mechanisms into an authenticated operator
workflow rather than presenting buttons without effects.

## What Changes

- Add an explicitly configured HTTPS web service for allowlisted repository
  enrollment, exact-source run initiation, live run/node observation, real log
  reads, artifact upload/download and exact-identity cancel/pause/resume.
- Reuse the existing Git and Lattice adapters, core decisions and observation
  admission. Add a bounded source-job helper for a real isolated worker.
- Keep the existing self-contained dashboard read-only and unchanged. Separate
  operator and worker credentials; no hosted accounts or ambient authority.

## Impact

- **Files**: new `crates/kiln-web-service`, typed `config/web-service.ncl` and
  `config/web-worker.ncl`, fixtures, workspace/package exports, operator and
  dashboard documentation, and this lifecycle package.
- **Testing**: paired pure admission and durable-state boundary tests, actual
  HTTPS/browser exercise against an isolated real Lattice workflow and source
  checkout, artifact identity checks, cancellation/stale-control/restart cases,
  focused Cargo checks and required repository gates.
- **Maintenance owner**: Kiln application/dashboard and provider-adapter maintainers.
- **Repeatability**: preserve exact executable/profile/source identities and a
  repository-owned local exercise. This does not activate production services,
  mutate forges, accept/archive specs, publish sources or grant release approval.
