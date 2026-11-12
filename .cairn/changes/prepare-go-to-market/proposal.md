## Why

Kiln is public on Radicle with a read-only GitHub projection, and `docs/commercial-launch.md` selects open-source Kiln funded by paid hosted CI. The repository is not yet a launch surface. The README opens with a 45-item status ledger and publication receipts before the quick start. There is no security policy, contribution guide, code of conduct or changelog. No positioning, release notes, launch checklist, demo or design-partner material is grounded in what the tree implements. The operator requested go-to-market preparation of this repository.

## What Changes

- Rework `README.md` into the landing surface: positioning, exercised quick start, capability map, hosted-CI direction and claim boundary. Relocate the detailed implemented-area ledger, integration procedures and publication record verbatim to `docs/status.md`.
- Add `docs/README.md` as the documentation index.
- Add `SECURITY.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` and `CHANGELOG.md`.
- Add the `docs/gtm/` launch kit: positioning, launch plan, launch checklist with observed and open items, 0.1.0 release-notes draft, announcement draft, outreach copy, demo script and the design-partner discovery guide for the hosted-CI pilot.
- Repair three broken links that predate this change: the Choregraph reference in the README, one anchor in `docs/hosted-ci.md` and one archived-evidence path in `docs/operator.md`.
- Apply the canonical Nickel formatter to `config/github-ci.ncl` and `config/microvm-guest-surface.ncl`. Their format drift made `scripts/check-nickel-fixtures.sh` fail before any fixture ran. The change is whitespace only.
- Record document checks, exercised commands, gate results and observed public-channel state in this package.

No runtime behavior, configuration meaning, accepted specification, license, lock file, deployment, remote repository setting, tag, release, announcement or payment-provider state changes. The missing vendored license texts found during this work are repaired by the separate package `retain-heartwood-license-texts`.

## Impact

- **Current consumer:** Kiln's operator preparing the 0.1.0 open-source launch and invite-only hosted-CI design-partner discovery, gate L0 in `docs/commercial-launch.md`. External demand remains unvalidated.
- **Immediate outcome:** a public surface that states what Kiln is, how to try it and what it does not claim.
- **Durable capability:** a repository-owned launch kit and go/no-go checklist keep external copy bound to retained evidence and name every owner-only action.
- **Owner:** Kiln maintainers own the README, community files and kit. The operator owns tags, publication, repository settings, outreach, company, prices and pilot activation.
- **Adoption:** the operator closes the checklist's hard gates, then tags and announces. Each later release refreshes the changelog, release notes and observed-state rows.
- **Repeatability:** the checklist records dated commands and observations; link checks and lifecycle gate outputs are retained under `evidence/`.
- **Files**: `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `docs/README.md`, `docs/status.md`, `docs/gtm/*.md`, one line each in `docs/hosted-ci.md` and `docs/operator.md`, one pointer sentence in `docs/commercial-launch.md`, formatting in `config/github-ci.ncl` and `config/microvm-guest-surface.ncl`, and this package.
- **Testing**: validate proposal, design and tasks gates and the no-spec classification; run `cairn validate`; check every relative link and anchor in the repository documents; run the documented quick-start and demo commands and the documented local gates, recording each result; review launch copy against `kiln.public_release.claims` with positive and negative claim cases. No runtime tests for prose.
- **No accepted-spec effect:** documentation, community files and launch copy add no product requirement. Hosted-CI qualification, payment, isolation and release gates keep their existing owners, specifications and evidence.
