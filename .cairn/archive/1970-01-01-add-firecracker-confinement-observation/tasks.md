## Phase 1: Pure admission

- [x] [serial] Add the bounded `/proc/<pid>/status` parser and executable admission with positive and negative tests. r[kiln.microvm.confinement]
- [x] [serial] Add the `ConfinementObserved` lifecycle gate and ordering tests. r[kiln.microvm.confinement.lifecycle]

## Phase 2: Host shell

- [x] [serial] Add `NoNewPrivileges=yes` to the VMM unit and observe the attempt cgroup/process before the guest session for cold boot and warm restore. r[kiln.microvm.confinement.observation]
- [x] [serial] Record observed confinement in the VM receipt and fail closed on unavailable evidence. r[kiln.microvm.confinement.receipt]

## Phase 3: Verification

- [x] [serial] Update hosted documentation, run focused and workspace checks, and record evidence. r[kiln.microvm.confinement.nonclaims]
