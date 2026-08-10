## Phase 1: Contract pin

- [ ] [serial] Pin the Cairn revision, executable, licenses, policy schema, request schema, result schema, diagnostic schema, and receipt schema. r[kiln.cairn_gate.profile] r[kiln.cairn_gate.nonclaims]
- [ ] [depends:cairn-contract-pin] Add a typed Nickel profile with exact executable identity and named bounds. r[kiln.cairn_gate.profile]
- [ ] [parallel] Add profile drift and unsupported-command fixtures. r[kiln.cairn_gate.conformance]

## Phase 2: Pure gate core

- [ ] [serial] Implement pure gate request planning and candidate applicability binding. r[kiln.cairn_gate.request]
- [ ] [depends:cairn-gate-request] Implement pure verdict, finding, diagnostic, receipt, and outcome admission. r[kiln.cairn_gate.result]
- [ ] [parallel] Add candidate, policy, evidence, and tool substitution tests. r[kiln.cairn_gate.conformance]

## Phase 3: Shell

- [ ] [depends:cairn-gate-profile] Add `kiln-adapter-cairn` outside core dependency graphs. r[kiln.cairn_gate.profile] r[kiln.cairn_gate.nonclaims]
- [ ] [depends:cairn-gate-result] Implement bounded process execution, cancellation, output capture, and reconciliation. r[kiln.cairn_gate.result] r[kiln.cairn_gate.reconciliation]
- [ ] [parallel] Add runtime independence and missing-executable tests. r[kiln.cairn_gate.nonclaims]

## Phase 4: Evidence

- [ ] [serial] Add pinned Cairn producer and independent Kiln consumer fixtures. r[kiln.cairn_gate.conformance]
- [ ] [serial] Document applicability, authority, reruns, diagnostics, and non-claims. r[kiln.cairn_gate.profile] r[kiln.cairn_gate.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.cairn_gate.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.cairn_gate.conformance] r[kiln.cairn_gate.nonclaims]
