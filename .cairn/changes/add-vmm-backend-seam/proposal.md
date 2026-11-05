## Why

Issue #32 needs a backend seam before Cloud Hypervisor can be added without
matching a closed runner enum in every consumer.

## What Changes

- Add the pure `kiln_microvm::backend::VmmBackend` trait with capability,
  boot-argument, listener and identity declarations.
- Implement Firecracker and a Cloud Hypervisor phase-1 plan surface.
- Add the exact Cloud Hypervisor cold-boot argv with no `--net` and no API
  socket.
- Use the Firecracker trait kind in the receipt.
- Drive backend arch, snapshot and no-network admission from the capability
  declaration.
- Add an explicit optional `network` request field and refuse anything other
  than `none`.
- Expose capability helpers for reviewed arches and snapshot admission.
- Add reviewed VMM version, receipt `backend_version`/`vmm_blake3`, and
  snapshot identity v2 binding kind/version/binary.

## Impact

- **Files**: `crates/kiln-microvm/src/backend.rs`,
  `crates/kiln-microvm/src/lib.rs`, `crates/kiln-hosted/src/microvm.rs`.
- **Testing**: capability, listener, boot-argument and argv tests.
- **Authority**: planning seam only. Cloud Hypervisor execution, admission
  wiring and the escape suite remain open.
