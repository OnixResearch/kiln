# Tasks: Local CI Composition E2E

## Phase 1: Harness

- [x] [serial] Add a `protocol-fixtures` integration test module with a bare Git source, pinned job profile, pinned resolver script, loopback service profile, and temporary SQLite. r[kiln.github.local_e2e]
- [x] [serial] Implement a closed Lattice Unix exchange handler that runs the real `kiln-github-job` on dispatch and returns the terminal contract observation. r[kiln.github.local_e2e]
- [x] [serial] Implement a recording `StatusPort` and raw HMAC HTTP driver. r[kiln.github.local_e2e]
- [x] [serial] Add a `protocol-fixtures`-only loopback status endpoint selection for the real binary. r[kiln.github.local_e2e.process]

## Phase 2: Scenarios

- [x] [serial] Prove a passing reviewed command becomes `Success` with one published success status and one job execution. r[kiln.github.local_e2e.outcomes]
- [x] [serial] Prove a nonzero reviewed command becomes `DomainFailure` with one published failure status. r[kiln.github.local_e2e.outcomes]
- [x] [serial] Prove an equal delivery reuses the run and does not execute a second job. r[kiln.github.local_e2e.recovery]
- [x] [serial] Prove restart recovery completes the original execution through `RecoverDispatch` without a new dispatch. r[kiln.github.local_e2e.recovery]
- [x] [serial] Assert the job sees no `.git`, no `GITHUB_TOKEN`, and no `HOME`. r[kiln.github.local_e2e.outcomes]
- [x] [serial] Prove the real binary composes the fixture status endpoint, real job helper, and real Lattice transport. r[kiln.github.local_e2e.process]
- [x] [serial] Prove killing and restarting the real binary recovers the original execution without a second job. r[kiln.github.local_e2e.process]
- [x] [serial] Prove the real binary rejects an invalid signature before any run, job, or status effect. r[kiln.github.local_e2e.process]
- [x] [serial] Prove the real binary handles a genuine nonzero command and an equal duplicate delivery. r[kiln.github.local_e2e.outcomes] r[kiln.github.local_e2e.recovery]
- [x] [serial] Prove an ambiguous status POST is reconciled by exact read-only lookup without reposting. r[kiln.github.local_e2e.status_recovery]
- [x] [serial] Prove duplicate critical headers, an oversized body, and an unsupported method are rejected before effects. r[kiln.github.local_e2e.framing]
- [x] [serial] Prove a changed profile against existing replay state fails closed without rewriting the original run. r[kiln.github.local_e2e.profile_drift]
- [x] [serial] Prove same-repository PR head selection, fork denial, and closed-PR suppression. r[kiln.github.local_e2e.events]
- [x] [serial] Prove partial requests time out and connection saturation returns capacity without effects, then recover. r[kiln.github.local_e2e.shell_bounds]

## Phase 3: Verification

- [x] [serial] Run the focused test under `protocol-fixtures`, the existing GitHub fixture tests, and strict Clippy. r[kiln.github.local_e2e]
- [x] [serial] Re-run `nix build path:$PWD#checks.x86_64-linux.github-protocol-fixtures -L` after all local E2E additions. r[kiln.github.local_e2e]
- [x] [serial] Record exact commands, results, and local-fixture non-claims in `evidence/verification.md`. r[kiln.github.local_e2e.nonclaims]
