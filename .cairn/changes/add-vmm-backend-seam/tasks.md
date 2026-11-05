## Phase 1: Seam

- [x] [serial] Add the pure VMM backend trait and capability declarations. r[kiln.microvm.backend.seam]
- [x] [serial] Implement Firecracker and the Cloud Hypervisor phase-1 plan surface. r[kiln.microvm.backend.seam]

## Phase 2: Evidence

- [x] [serial] Test no-network capabilities, shared listener paths and CH argv. r[kiln.microvm.backend.seam]
- [x] [serial] Record the Firecracker backend kind from the trait. r[kiln.microvm.backend.seam]
- [x] [serial] Drive arch, snapshot and no-network admission from capabilities. r[kiln.microvm.backend.seam]
- [x] [serial] Refuse explicit non-`none` network requests. r[kiln.microvm.backend.seam]
- [x] [serial] Test reviewed-arch and snapshot-capability refusal. r[kiln.microvm.backend.seam]
- [x] [serial] Record VMM version/binary and bind snapshot identity v2. r[kiln.microvm.backend.seam]
- [x] [serial] Add Cloud Hypervisor pause/snapshot/restore plans. r[kiln.microvm.backend.seam]
- [x] [serial] Document phase-1 claims and non-claims. r[kiln.microvm.backend.seam]
- [x] [serial] Build `microvm-guest-ch` with virtio-PCI and emit guest-image v2. r[kiln.microvm.backend.seam]
- [x] [serial] Wire Cloud Hypervisor cold-boot execution and configuration admission. r[kiln.microvm.backend.seam]
- [x] [serial] Generalize confinement process-name observation. r[kiln.microvm.backend.seam]
- [x] [serial] Aggregate thread confinement and encode the CH control-thread policy. r[kiln.microvm.backend.seam]
- [x] [serial] Run the CH cold-boot pipeline, warm-pool restore and escape suite on the host. r[kiln.microvm.backend.seam]
