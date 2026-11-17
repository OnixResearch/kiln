## Why

Issue [#84](https://github.com/OnixResearch/kiln/issues/84): an operator cannot open one Cairn change and see its intent, execution and remaining delivery obligations together. Canonical implementation work, attempts, verification and publication records, native lifecycle receipts and selected campaigns each record part of the picture. The console shows campaigns on the Workers page and canonical work only through inventory explanations. Nothing joins them by the exact Cairn change and task identity, so blocked, planning-only and completion-only members, historical evidence and the next resolving operation are hard to read.

## What Changes

- Add a read-only canonical snapshot to `kiln_implementer::observation`. One read transaction covers the work and attempt ledgers, native Cairn lifecycle receipts and selected campaigns, with explicit read bounds and truncation.
- Add a pure Cairn workspace projection in `kiln-web-service`, keyed by repository identity (BLAKE3 of the repository binding, as in the campaign projection) and Cairn change. It has two parts:
  - **Index:** every change referenced by canonical work aliases or campaign selections, with projects, unit, attempt, campaign and blocker counts.
  - **Detail:** units (native task identities, canonical work versions, readiness snapshot, acceptance obligations and dependencies), attempts with evidence applicability, verification and delivery records, lifecycle receipts, campaign memberships with their holds and pending effects, decisions, and structured blockers. Each unit keeps readiness, execution, evidence, integration and lifecycle separately legible.
- Add `GET /api/implementation/cairn` and `GET /api/implementation/cairn/detail?repository=ID&change=NAME` over `--implementation-work`. Viewers and operators read; worker principals are refused.
- Add a **Cairn changes** console page (2.7) with a filterable change list and a detail view bound to the URL. Responses for another selection never overwrite the shown one. Campaign members link to their Cairn change.
- Extend the exploration workspace with canonical work and attempts beside its synthetic campaign: A→B dependency, independent C with a superseded attempt and a stale campaign binding, completion-only D, and a review wait.

## Impact

- **Consumer:** operators following selected Cairn changes; campaign controls (#86) and typed snapshots (#83) later attach to the same detail view.
- **Immediate outcome:** one page shows a Cairn change's units, attempts, evidence, decisions, delivery and remaining blockers with their owners.
- **Durable capability:** a bounded canonical snapshot reader and a pure join that later typed Cairn snapshots and capability scopes extend.
- **Owner:** Kiln web-service and implementation maintainers.
- **Files:** `crates/kiln-implementer/src/observation.rs`, `crates/kiln-web-service/src/{cairn_workspace.rs,cairn_workspace/tests.rs,lib.rs,server.rs,cairn.js,cairn.css,ui.js,web.html,web.js,inventory.js}`, `crates/kiln-web-service/examples/{explore.rs,explore/campaign.rs}`, `tests/cairn-ui.test.mjs`, `docs/operator.md`, `CHANGELOG.md`, `cairn-policy/development-coverage.ncl` and this package.
- **Verification:** implementer and web-service unit tests over real SQLite stores, Node UI tests, a browser review of the exploration workspace, workspace and Nix gates, Cairn gates, validation and traceability.
- **Authority:** working-copy implementation and verification only; no deployment or live campaign.

## Out of Scope

Capability-restricted observers and per-holder disclosure (#69/#70/#75/#76/#79): every authenticated viewer or operator sees the same projection, as for the campaign projection. Typed Cairn requirement and acceptance-criterion references and native obligation explanations (#83, which needs Cairn #14). Requirement text beyond canonical acceptance obligations. Mutating controls (#86 owns them). The Rats/Meteorite renderer (#67/#68).
