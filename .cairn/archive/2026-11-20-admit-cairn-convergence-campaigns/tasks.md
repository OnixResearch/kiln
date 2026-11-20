## Phase 1: Intake, selection and campaign

- [x] [serial] Implement the pure bounded `cairn.convergence.v1` intake with typed rows, stable issue codes and receipt/registry/case/policy identity binding. r[kiln.convergence.intake]
- [x] [serial] Implement unsatisfied-set selection with explicit exclusions for `current`, `not_projected` and `untargeted`. r[kiln.convergence.unsatisfied]
- [x] [serial] Implement one frozen campaign revision bound to the candidate-set BLAKE3, with deterministic identity and empty/duplicate refusals. r[kiln.convergence.campaign]

## Phase 2: Verify requests and evidence

- [x] [serial] Implement typed Verify-stage evidence build requests with capability and realization admission before effects, lowering to `BuildEffectPlan`. r[kiln.convergence.verify]
- [x] [serial] Retain the pilot receipt and synthetic negatives; run the focused tests, clippy and fmt, then the Cairn validate and gates and record the evidence. r[kiln.convergence.nonclaims]
