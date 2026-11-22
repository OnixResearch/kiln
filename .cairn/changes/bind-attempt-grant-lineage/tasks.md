## Phase 1: Lineage core

- [x] [serial] Add the `kiln.attempt-authority-binding.v1` types, deterministic BLAKE3 ceiling, intent and binding identities, and `admit_root` with positive and negative fixtures. r[kiln.attempt.grant_lineage.root] r[kiln.attempt.grant_lineage.root.scenario.refused] r[kiln.attempt.grant_lineage.identity] r[kiln.attempt.grant_lineage.identity.scenario.deterministic]
- [x] [serial] [depends:kiln.attempt.grant_lineage.root] Add `admit_child` with a refusal fixture for every `LineageRefusal` variant and exhaustive attenuation properties. r[kiln.attempt.grant_lineage.attenuation] r[kiln.attempt.grant_lineage.attenuation.scenario.permitted] r[kiln.attempt.grant_lineage.attenuation.scenario.refused] r[kiln.attempt.grant_lineage.attenuation.scenario.property] r[kiln.attempt.grant_lineage.foreign] r[kiln.attempt.grant_lineage.foreign.scenario.refused] r[kiln.attempt.grant_lineage.budget] r[kiln.attempt.grant_lineage.budget.scenario.refused]
- [x] [serial] [depends:kiln.attempt.grant_lineage.attenuation] Add `check_current` so revocation, expiry, a moved generation or a replaced fence on any ancestor invalidates every descendant. r[kiln.attempt.grant_lineage.currentness] r[kiln.attempt.grant_lineage.currentness.scenario.propagate]

## Phase 2: Attempt field and issuance fold

- [x] [serial] [depends:kiln.attempt.grant_lineage.root] Add the optional attempt `authority` field and the `bind_authority` fold, absent from serialization until bound. r[kiln.attempt.grant_lineage.attempt] r[kiln.attempt.grant_lineage.attempt.scenario.bound]
- [x] [serial] [depends:kiln.attempt.grant_lineage.currentness] Add the unwired issuance record types and the pure reconciliation fold. r[kiln.attempt.grant_lineage.issuance] r[kiln.attempt.grant_lineage.issuance.scenario.recover]

## Phase 3: Evidence

- [x] [serial] Draft the Basalt B1 request, update the changelog, and run the focused tests, clippy and Cairn gates (`evidence/rust-checks.txt`, `evidence/gates.txt`). r[kiln.attempt.grant_lineage.attenuation]
