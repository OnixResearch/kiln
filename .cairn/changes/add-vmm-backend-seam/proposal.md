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
- Add reviewed VMM version, receipt `backend_version`/`vmm_blake3`, declared
  capabilities and snapshot identity v2 binding kind/version/binary.
- Add Cloud Hypervisor pause/snapshot requests and restore argv using the
  pinned API schema.
- Document Cloud Hypervisor phase-1 claims and non-claims.
- Build a virtio-PCI `microvm-guest-ch` image and move the guest manifest to
  `kiln.microvm.guest-image.v2` with a `vmm` entry.
- Wire Cloud Hypervisor cold-boot execution into the hosted shell; warm restore
  remains unwired.
- Generalize the confinement process-name check to the backend kind and Linux's
  15-byte `comm` truncation.
- Aggregate per-thread confinement facts and allow Cloud Hypervisor's unfiltered
  control thread only when at least one worker thread is seccomp-filtered.

## Impact

- **Files**: `crates/kiln-microvm/src/backend.rs`,
  `crates/kiln-microvm/src/lib.rs`, `crates/kiln-hosted/src/microvm.rs`.
- **Testing**: capability, listener, boot-argument and argv tests.
- **Authority**: planning seam only. Cloud Hypervisor execution, admission
  wiring and the escape suite remain open.
