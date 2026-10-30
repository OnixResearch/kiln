## Phase 1: Core

- [x] [serial] Move the shared glob matcher into `kiln_core::paths`, and add `kiln_core::monorepo` with scopes, changed-file facts, the affected decision and identities. r[kiln.monorepo.scope] r[kiln.monorepo.affected] r[kiln.monorepo.affected.scenario.graph] r[kiln.monorepo.affected.scenario.fallback]
- [x] [serial] Add the pure `kiln-build-graph` crate for Cargo workspace and Nix flake path-input graphs. r[kiln.monorepo.graph]

## Phase 2: Shell and console

- [x] [serial] Add scope admission to the service profile and the Nickel contract, compute changed files from Git, read graph manifests at the exact revision, dispatch only affected projects, set the job working directory, and record the affected decision. r[kiln.monorepo.scope.scenario.invalid] r[kiln.monorepo.changed] r[kiln.monorepo.dispatch]
- [x] [serial] Bind scope identity in gates, and require CI per affected project in review. r[kiln.monorepo.gating]
- [x] [serial] Console: group projects by repository, show roots and graphs, add "Why this ran", show trigger scope and per-project review CI. r[kiln.monorepo.console]

## Phase 3: Verification

- [x] [serial] Run the tests, the real-history affected check on this repository, a disposable-instance browser review, the repository gates, documentation and deployment. r[kiln.monorepo.scope] r[kiln.monorepo.changed] r[kiln.monorepo.graph] r[kiln.monorepo.affected] r[kiln.monorepo.dispatch] r[kiln.monorepo.gating] r[kiln.monorepo.console]

Evidence: `evidence/verification.json` and `evidence/console.json`. Accepted on operator request 2026-10-30.
