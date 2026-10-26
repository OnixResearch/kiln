## Phase 1: Runtime and deployment

- [x] [parallel] Add non-mutating preflight, readiness/health and private runtime credential admission with paired boundary tests. r[kiln.web.deploy.runtime]
- [x] [parallel] Add hardened typed deployment assets, dedicated packaging and supported system assembly with explicit operator inputs. r[kiln.web.deploy.install]
- [x] [parallel] Correct cancellation in the owning Lattice producer and retain an exact measured consumer cohort without outcome relabeling. r[kiln.web.deploy.cancellation]

## Phase 2: Qualification

- [x] [serial] Exercise actual supervised HTTPS startup, private credentials, health, stop/restart, bad-input rejection and source-bound workflow success/failure/logs/artifacts/cancellation. r[kiln.web.deploy.evidence]
- [ ] [serial] Run focused and required gates, document installation, credential rotation, recovery and owner boundaries, and retain scoped evidence. r[kiln.web.deploy.evidence]

## Scoped verification status

The real packaged systemd VM rail, supported Onix/preservation consumer assembly,
complete pure Onix shim suite and complete Kiln flake check pass. Evidence is in
`evidence/verification.json` and `evidence/consumer-assembly.json`. Onix's native
preservation boundary correction has its own owner-local change and receipts.

The corrected complete Lattice executable now builds with Tile, shared-runtime,
Mantle, Basalt and actual pinned Valence integrations enabled. The unchanged
strict native rail passes for the recorded immutable cohort, including canonical
`Cancelled` after the actual command starts. Restart preserves its execution
without redispatch, and logs/artifacts/authentication/TLS/control checks pass.
`evidence/native-corrected/verification.json` records exact identities and complete
process, credential and scratch retirement. Earlier failures remain historical.

The required-gates task remains open: Lattice's broader unit/TUI/Octet/Nix checks
are not green, and the earlier Kiln traceability/release-readiness check failed
on nine dashboard/Patchgate references. No acceptance, archive or host activation.
