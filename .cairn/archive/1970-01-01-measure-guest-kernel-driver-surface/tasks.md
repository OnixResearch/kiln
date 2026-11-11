## Phase 1: Policy and measurement

- [x] [serial] Add the reviewed enforced/measured-only deny list with exact Kconfig symbols and module fragments. r[kiln.hosted.microvm.guest_surface]
- [x] [serial] Add the bounded inventory measurement over the pinned kernel configuration and guest module closure. r[kiln.hosted.microvm.guest_surface]

## Phase 2: Enforcement and evidence

- [x] [serial] Fail the guest-image check on an enforced builtin or shipped module and compare the inventory with the checked-in fixture. r[kiln.hosted.microvm.guest_surface.scenario.absent] r[kiln.hosted.microvm.guest_surface.scenario.violation]
- [x] [serial] Add fail-closed enforced-class sysfs probes to the guest escape suite. r[kiln.hosted.microvm.guest_surface.scenario.escape]
- [x] [serial] Record the measured verdicts and residual risk in the hosted documentation; keep `hostile_isolation_claim` false. r[kiln.hosted.microvm.guest_surface]
