## Phase 1: Derivation and routes

- [x] [serial] Derive `lattice.kiln-budget-lineage.v1` from the admission ledger (root, work account, committed reservations), mapped field for field to Lattice `5a8e1d51`, with typed `LineageRefusal` reasons. r[kiln.admission.lattice_lineage]
- [x] [serial] [depends:kiln.admission.lattice_lineage] Append the lineage on the `--worker-request` route before the owner-grant gate, freeze the dispatched bytes, recover only from the frozen request, and name it in campaign export. r[kiln.admission.lattice_lineage.scenario.carried]
- [x] [serial] [depends:kiln.admission.lattice_lineage] Check the same lineage on the `--dispatch-plan` exchange route without changing its `lattice.workflow-request` bytes. r[kiln.admission.lattice_lineage.scenario.exchange]

## Phase 2: Verification

- [x] [serial] [depends:kiln.admission.lattice_lineage.scenario.carried] Add golden request bytes with and without lineage, acceptance by Lattice's published schema as data, both routes, the durable worker-route path and every refusal. r[kiln.admission.lattice_lineage.scenario.refused]
- [x] [serial] Update `docs/fair-admission.md`, `docs/implementation-runbook.md` and `CHANGELOG.md`, and record the focused gates and Lattice-code acceptance as evidence. r[kiln.admission.lattice_lineage]

## Phase 3: Review fixes

- [x] [serial] Require a held amount only in dimensions Lattice charges and omit uncharged dimensions the attempt does not hold, so shared-allocation work is admitted; require every required owner's committed grant. r[kiln.admission.lattice_lineage]
- [x] [serial] [depends:kiln.admission.lattice_lineage.scenario.carried] Freeze the sent bytes under their own BLAKE3, record the file in `TransportLaunch.request`, read exactly that file on recovery and campaign export, and word recovery refusals as unknown outcomes. r[kiln.admission.lattice_lineage.scenario.carried]
- [x] [serial] Find Lattice's intent by the sent digest in the qualification crash barrier and retain the sent request as stage evidence; update `docs/implementation-qualification.md`. r[kiln.admission.lattice_lineage.scenario.carried]
- [x] [serial] [depends:kiln.admission.lattice_lineage.scenario.refused] Test the request choice, the exchange check, the export path and every derivation bound, and confirm by mutation that removing each new check fails a test. r[kiln.admission.lattice_lineage.scenario.refused]
