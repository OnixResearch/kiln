## Phase 1: Implementation

- [x] [serial] Move the campaign export's request and state resolver into `kiln_adapter_lattice::worker` as `sent_request_and_state` and call it from the campaign export. r[kiln.admission.lattice_lineage.scenario.carried]
- [x] [serial] Add `Implementer::open_read_only` and the one-snapshot `Implementer::dispatch_record`. r[kiln.admission.lattice_lineage.scenario.reported]
- [x] [serial] [depends:kiln.admission.lattice_lineage.scenario.reported] Add `kiln-lattice-dispatch --sent-request true`, which prints one verified `kiln.lattice-sent-request.v1` object or refuses with empty stdout, and name it in the dispatcher usage, `sent_request_path` and the operator docs. r[kiln.admission.lattice_lineage.scenario.reported]

## Phase 2: Verification

- [ ] [serial] [depends:kiln.admission.lattice_lineage.scenario.reported] Test the recorded lineage-bearing request, the legacy launch, every refusal, empty stdout on refusal, and that no ledger version, ledger byte or file changes; run the focused tests, workspace clippy, fmt, Cairn gates and `nix flake check`, and record them as evidence. r[kiln.admission.lattice_lineage.scenario.reported]
