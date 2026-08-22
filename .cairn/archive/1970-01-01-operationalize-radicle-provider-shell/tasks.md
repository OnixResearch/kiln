## Phase 1: Profile and boundary

- [x] [serial] Pin the supported Radicle runtime, Git storage, protocol, and license cohort. r[kiln.radicle.shell_profile] r[kiln.radicle.shell_nonclaims]
- [x] [depends:radicle-runtime-pin] Add typed Nickel profiles for source and trigger capabilities. r[kiln.radicle.shell_profile]
- [x] [parallel] Add dependency gates that keep Radicle runtime code outside core and standalone crates. r[kiln.radicle.shell_nonclaims]

## Phase 2: Pure decisions

- [x] [serial] Add pure source and trigger observation, effect, outcome, and reconciliation decisions. r[kiln.radicle.shell_source] r[kiln.radicle.shell_trigger]
- [x] [parallel] Add positive and negative tests for substitution, drift, replay, stale heads, and unknown outcomes. r[kiln.radicle.shell_conformance]

## Phase 3: Concrete shell

- [x] [depends:radicle-profile] Implement bounded local repository and trigger intake shells. r[kiln.radicle.shell_source] r[kiln.radicle.shell_trigger]
- [x] [parallel] Move `LocalFixtureService` to test-only or example-only code. r[kiln.radicle.shell_conformance]

## Phase 4: Evidence and completion

- [x] [serial] Add local producer and independent consumer fixtures for every enabled capability. r[kiln.radicle.shell_conformance]
- [x] [serial] Document setup, authority, recovery, limits, and non-claims. r[kiln.radicle.shell_profile] r[kiln.radicle.shell_nonclaims]
- [x] [serial] Run formatting, workspace tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and focused Nix gates. r[kiln.radicle.shell_conformance]
- [x] [serial] Record evidence, sync accepted specs, archive the change, and rerun validation. r[kiln.radicle.shell_conformance] r[kiln.radicle.shell_nonclaims]
