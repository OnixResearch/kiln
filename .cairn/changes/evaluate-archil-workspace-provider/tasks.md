## Phase 1: Profile and documentation review

- [x] [serial] Add the versioned provider profile contract with positive local and Archil fixtures and one negative fixture per refusal, checked by a test of each refusal's reason in the nickel Nix check. r[kiln.workspace_provider.evaluation] r[kiln.workspace_provider.evaluation.scenario.refused] r[kiln.workspace_provider.evaluation.scenario.claims]
- [x] [serial] Write the documentation-review evaluation with every Archil capability labeled as a vendor claim and the adopt, adapt or reference decision with its unresolved conditions. r[kiln.workspace_provider.evaluation]

## Phase 2: Corpus and live comparison

- [x] [serial] Build the shared corpus and run the local baseline through #78's projected materializer, retaining the machine-readable comparison. r[kiln.workspace_provider.evaluation] r[kiln.workspace_provider.evaluation.scenario.baseline]
- [ ] [serial] Operator runs the live Archil phase in a disposable environment with named time, resource, cost and retention bounds, retaining raw timing, fault and cleanup receipts. r[kiln.workspace_provider.evaluation]
