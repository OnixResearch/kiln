## 1. Plan

- [x] [serial] Map every #51–#55 acceptance criterion to a milestone with multi-host, onix-core, Aspen, Lattice and infrastructure prerequisites in `milestones.md`. r[kiln.lattice.worker.guest]

## 2. Pure core and guest shell

- [x] [serial] Add `kiln_microvm::lattice_worker` environment/manifest identities, host and guest admission, guest identity derivation, authority profile rendering and bounded dispatch frames, with positive and negative tests for every crossed identity and an expired deadline. r[kiln.lattice.worker.guest.scenario.refused] r[kiln.lattice.worker.guest.scenario.isolated]
- [x] [serial] Add the `kiln-lattice-worker-guest` shell: blank-workspace formatting and labelling, single dispatch, guest-side admission, per-attempt Lattice config/authority, uid-1000 Lattice execution and bounded report. r[kiln.lattice.worker.guest.scenario.executed]

## 3. Packaging

- [x] [serial] Add the Lattice cohort subflake and the x86_64-linux `lattice-worker-guest` package with build-time effect requirements and the `kiln.lattice-worker-guest.v1` manifest; leave per-job guests unchanged. r[kiln.lattice.worker.guest] r[kiln.lattice.worker.guest.scenario.identity]

## 4. Hosted shell

- [x] [serial] Add `kiln_hosted::microvm::lattice_worker` manifest verification, workspace creation, Running-gated journalled dispatch with peer-PID check and no redispatch, with positive and negative unit tests. r[kiln.lattice.worker.session] r[kiln.lattice.worker.session.scenario.dispatch-once]

## 5. Single-host KVM qualification

- [x] [serial] Add the real-KVM test through the existing retained shell with a test-only reservation: cold boot, resume, two isolated dispatches, guest-side refusal, pause, snapshot, retire, reopen, restore and continuation; stale and retired owner refusals; controller restart without redispatch; explicit skip/required failure. r[kiln.lattice.worker.session.scenario.lifecycle] r[kiln.lattice.worker.session.scenario.stale] r[kiln.lattice.worker.session.scenario.unavailable] r[kiln.lattice.worker.guest.scenario.executed]
- [x] [serial] Record KVM evidence separately from fixture evidence, update the retained-session runbook and status, and run the repository gates. r[kiln.lattice.worker.session]

## 6. Later milestones (not this change)

- [ ] [serial] M2–M5 per `milestones.md`: admitted placement/fencing provider, onix-core service integration, Lattice #10/#11, two-host paused and live relocation over Iroh/Aspen and draining. r[kiln.lattice.worker.session]
