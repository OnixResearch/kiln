## Phase 1: Pure core

- [x] [serial] Add `kiln-microvm` with machine admission bounds, VM plan, snapshot identity, source archive, lifecycle state machine, guest protocol and environment/argument parity, with positive and negative tests. r[kiln.hosted.microvm.admission] r[kiln.hosted.microvm.admission.scenario.bounds] r[kiln.hosted.microvm.plan] r[kiln.hosted.microvm.lifecycle] r[kiln.hosted.microvm.lifecycle.scenario.journal] r[kiln.hosted.microvm.protocol]

## Phase 2: Shells, image and configuration

- [x] [serial] Add the guest agent binary and the x86_64 `microvm-guest` flake output (kernel, initramfs, pinned Firecracker and manifest). r[kiln.hosted.microvm.protocol] r[kiln.hosted.microvm.escape]
- [x] [serial] Register `Backend::Firecracker` in hosted execution with the Firecracker shell, journal-before-destroy rows, the warm pool and `canonical_outcome`, plus Nickel contracts and fixtures. r[kiln.hosted.microvm.admission] r[kiln.hosted.microvm.lifecycle] r[kiln.hosted.microvm.warm_pool] r[kiln.hosted.microvm.equivalence]

## Phase 3: Verification and documentation

- [x] [serial] Add the KVM-gated host test for backend equivalence, the escape suite and latency, then run it on this host and record evidence. r[kiln.hosted.microvm.equivalence.scenario.host] r[kiln.hosted.microvm.escape.scenario.suite] r[kiln.hosted.microvm.warm_pool]
- [x] [serial] Document the upgraded and non-upgraded isolation claims in `docs/hosted-ci.md` and `README.md`. r[kiln.hosted.microvm.claims]

Evidence: `evidence/verification.json` (host test on this KVM host: canonical equivalence, guest-escape probes, cold-start and restore latency).

Accepted on operator request 2026-10-30 after parent integration review; see `evidence/verification.json`.
