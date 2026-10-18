## Phase 1: Dependency ownership

- [x] [serial] Inventory selected Cargo/Nix/import sources, nested Trellis ownership, and historical Octet source names. a[prepare-owned-dependencies-agpl.inventory]

## Phase 2: Owner preparation

- [x] [serial] Prepare all new first-party producer source under the operator-selected AGPL-3.0-or-later policy through repository-local gated changes. a[prepare-owned-dependencies-agpl.boundary]
- [x] [serial] Update Kiln's existing license map with the prospective boundary without relabeling immutable consumed snapshots. a[prepare-owned-dependencies-agpl.boundary]

## Phase 3: Verification

- [x] [serial] Run owner license/metadata/projection checks and applicable lifecycle gates from each actual target repository; retain exact outcomes and prerequisites. a[prepare-owned-dependencies-agpl.verification]
- [x] [serial] Verify Kiln's retained source-import boundary and record the scoped cross-repository readiness handoff without publication or legal/privacy claims. a[prepare-owned-dependencies-agpl.verification]

## Recorded boundary

See `evidence/final-verification.json`, `evidence/trellis-verification.json` and
`evidence/commands.json`. All twenty selected owners have local prospective
source preparation: fifteen producer changes and five already-AGPL owners.
Trellis's full-strength policy repair and source cutover are exercised; its
licensing native sync remains blocked by the accepted domain's pre-existing
multiple requirement IDs, with no legacy migration or archive authorized here.
Nix store installation, Rats source identity/notice review, and an explicitly
authorized Radicle COB Git Store remote remain prospective producer-release
prerequisites, not blanket blockers for Kiln's unchanged imported source.
`evidence/current-source-review.json` records their applicability and retained
source grants. Exact Kiln candidate acquisition, privacy/rights and applicable
source/combined-output review remain separate public-release prerequisites.
The checked preparation tasks do not assert release eligibility or publication.
