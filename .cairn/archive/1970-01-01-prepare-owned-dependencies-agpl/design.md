## Context

The root Cargo workspace, two standalone runtime locks, optional build-cohort lock, and immutable source records define the selected closure. Reusable verified-logic source is owned by Trellis even when carried by an immutable UCAN snapshot. Earlier producer grants remain applicable to Kiln's existing pins.

## Decisions

### Decision: Prospective AGPL cutover in each owning repository

**Choice:** Use AGPL-3.0-or-later for all new owned source, including cores, contracts, schemas, templates, and policy. Each owner updates its existing source-of-truth license map and verification. Preserve third-party material, historical grants, attribution, and immutable imported snapshots.

**Rationale:** A consumer-side notice cannot relicense producer code. Owner-maintained maps and executable checks make later releases repeatable without creating a second licensing system in Kiln.

### Decision: Keep current Kiln contracts and cohorts unchanged

**Choice:** Record preparation separately from the licenses of exact consumed revisions. Do not migrate Kiln to unpublished working-tree sources or revise locks. Use a demonstrated-compatible lifecycle executable with each owner's complete accepted policy; never weaken policy to accommodate a newer CLI.

**Rationale:** Source preparation does not establish a new immutable release, consumer compatibility, publication authority, or a legal conclusion about combined artifacts. This Kiln change only documents evidence; it changes no accepted runtime requirement or public API.

## Risks / Trade-offs

- Existing MIT/Apache/MPL and multilicensing grants cannot be revoked; byte-identical previously granted source remains available under those grants.
- New AGPL dependencies change downstream obligations. Kiln's existing GPL-2.0-only Pijul boundary and combined-output cautions remain unchanged.
- Missing compatible lifecycle tools, source access, imported-material permissions, or final candidate/privacy checks must remain explicit prerequisites, not passing readiness claims.
- Each producer maintainer owns its license policy and repeatability checks; Kiln maintainers own the selected-closure inventory and pinned-consumer boundary.
