## ADDED Requirements

### Requirement: Read-only Fleet local-copy census across configured machines

r[kiln.fleet.local_copy_census]
`kiln-fleet` MUST expose an authenticated read-only endpoint `GET /v1/repositories` (and `crates/kiln-web-service/src/fleet.rs` MUST proxy `GET /api/fleet/repositories` -> `/v1/repositories`) that observes local repository checkouts across configured fleet machines and checkout roots without mutating working trees, running remote fetches, or invoking Radicle mutations. Each observed copy MUST report `machine`, `path`, `vcs` (`git | jj | pijul`), exact `head_commit` (or `null`), `branch_or_bookmark`, `dirty` (`bool | null`), discovered remote URLs/RIDs, active `.cairn/changes` directory count, `observed_at_ms`, and `health`. Unreachable or unconfigured hosts MUST appear explicitly with `health: "unavailable"` and their error diagnostic rather than being omitted or reported as clean.

#### Scenario: Offline fleet host remains explicit alongside healthy local copies

r[kiln.fleet.local_copy_census.scenario.offline_host]
- GIVEN a fleet with local checkouts observed on `britton-desktop` and an unreachable peer host `aspen3`
- WHEN a client fetches `GET /api/fleet/repositories`
- THEN `britton-desktop`'s observed local copies MUST be returned with exact HEAD, VCS, remotes, and Cairn count
- AND `aspen3` MUST be reported explicitly as `unavailable` with its failure reason
