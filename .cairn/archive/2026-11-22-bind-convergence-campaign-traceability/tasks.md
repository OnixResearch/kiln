## Phase 1: Intake and selection evidence

- [x] [serial] Bind the Cairn convergence intake requirement and its five scenarios to `admit_convergence_receipt` and the adapter tests that check schema, identity, size, pilot and current-row behaviour. a[bind-convergence-campaign-traceability.markers]
- [x] [serial] Bind unsatisfied selection and its scenarios to `select_unsatisfied` and the selection tests, and add the missing moved-or-unbound-target test. a[bind-convergence-campaign-traceability.markers]

## Phase 2: Campaign and Verify evidence

- [x] [serial] Bind campaign minting and its four scenarios to `mint_evidence_campaign` and the mint tests. a[bind-convergence-campaign-traceability.markers]
- [x] [serial] Bind Verify request planning and its five scenarios to `plan_evidence_builds` and the planning tests. a[bind-convergence-campaign-traceability.markers]
- [x] [serial] Record the convergence non-claims as a module constant, bind the requirement to it, and add the advisory test. a[bind-convergence-campaign-traceability.markers]

## Phase 3: Realize evidence

- [x] [serial] Bind preserve-route realization to the request lowering, the plan-keyed realize profile binding and the closed-world completion admission, and bind its pilot and substitution scenarios to the Mantle adapter tests. a[bind-convergence-campaign-traceability.markers]
- [x] [serial] Bind the realize profile, dispatch, completion and recovery requirements and their scenarios to the exact adapter functions and tests, and add the pinned-profile and release-non-claim tests. a[bind-convergence-campaign-traceability.markers]

## Phase 4: Gates and closure

- [x] [serial] Run fmt, clippy, the three crate test suites, `cairn validate`, `cairn traceability coverage` and `cairn release-readiness` inside the pinned Nix shell, record the transcripts, then sync and archive the change. a[bind-convergence-campaign-traceability.gates]
