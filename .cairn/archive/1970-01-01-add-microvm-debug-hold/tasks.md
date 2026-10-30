## Phase 1: Core

- [x] [serial] Add `kiln_microvm::debug_hold`: policy admission, eligibility, failure classification, hold end decision, capability digest and redemption, debug step plan and connection instructions. r[kiln.hosted.microvm.debug_hold.policy] r[kiln.hosted.microvm.debug_hold.capability] r[kiln.hosted.microvm.debug_hold.end]
- [x] [serial] Add `Held`, `Hold`, `HoldEnded` and `Effect::Hold` to the lifecycle, reachable only after `Journaled` and a completed guest session. r[kiln.hosted.microvm.debug_hold.lifecycle] r[kiln.hosted.microvm.debug_hold.lifecycle.scenario.reachability] r[kiln.hosted.microvm.debug_hold.end.scenario.expiry] r[kiln.hosted.microvm.debug_hold.policy.scenario.disabled]
- [x] [serial] Extend the bounded protocol and the guest agent with a debug request and report, accepted only after a failed run. r[kiln.hosted.microvm.debug_hold.relay]

## Phase 2: Shell

- [x] [serial] Add `debug_hold` to the Firecracker backend, Rust admission and the Nickel contracts with fixtures. r[kiln.hosted.microvm.debug_hold.policy] r[kiln.hosted.microvm.debug_hold.policy.scenario.bounds]
- [x] [serial] Hold the VM in the Firecracker shell, relay debug sessions over the guest stream, extend unit time and admission by the grace, record capacity pressure, and end holds into destruction. r[kiln.hosted.microvm.debug_hold.end] r[kiln.hosted.microvm.debug_hold.relay]
- [x] [serial] Add the `execution.debug_issue`, `debug_exec`, `debug_release` and `debug_list` actions, session records, and connection instructions in `execution.get` and receipts. r[kiln.hosted.microvm.debug_hold.capability] r[kiln.hosted.microvm.debug_hold.capability.scenario.single_use] r[kiln.hosted.microvm.debug_hold.capability.scenario.write_access] r[kiln.hosted.microvm.debug_hold.sessions]

## Phase 3: Verification

- [x] [serial] Run the focused crate tests, Nickel fixtures and the KVM host test with recorded timings, then document the option in `docs/hosted-ci.md`. r[kiln.hosted.microvm.debug_hold.relay.scenario.host] r[kiln.hosted.microvm.debug_hold.sessions]
