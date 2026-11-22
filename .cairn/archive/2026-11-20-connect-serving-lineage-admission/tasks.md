## Phase 1: Core

- [x] [serial] Add the opt-in serving policy, `ServingObservation`, monotonic retention and the serving gate before claim and request, with the `overloaded` reason. r[kiln.admission.serving]
- [x] [serial] Add lineage revocation, ancestor blockers, `LineageUsage` conservation and lineage/serving explanation facts. r[kiln.admission.lineage]

## Phase 2: Shell and profiles

- [x] [serial] [depends:kiln.admission.serving] Add `observe_serving`, `revoke` and `lineage` implementer commands and the shell serving pre-check. r[kiln.admission.serving] r[kiln.admission.lineage]
- [x] [serial] Add Nickel serving-policy and recursive-lineage contracts, fixtures, committed exports and the staleness check. r[kiln.admission.profiles]

## Phase 3: Verification

- [x] [serial] [depends:kiln.admission.lineage] Add core and durable tests for last-slot, stale snapshot, recursive lineage, revocation, cancellation, restart and duplicate-request cases. r[kiln.admission.serving.scenario.last_slot] r[kiln.admission.serving.scenario.stale] r[kiln.admission.lineage.scenario.recursive] r[kiln.admission.lineage.scenario.blocked] r[kiln.admission.profiles.scenario.exports]
- [x] [serial] Update docs and changelog and run the repository gates without live claims. r[kiln.admission.profiles]
