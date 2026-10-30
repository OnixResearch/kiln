## Why

Issue #2 asks for a Firecracker microVM execution backend for hosted CI workers. The hosted composition already has explicit runner backends (`namespaces`, `qemu`) selected through the operator runner map and bounded by the `maximum_concurrent` slot model. The `namespaces` backend is trusted-only: a customer job shares the host kernel and is exposed to the unprivileged user-namespace kernel LPE class. A per-job microVM puts a separate guest kernel and a KVM boundary between the job and the host kernel.

## What Changes

- Add `kind = "firecracker"` to the hosted runner backends. It registers through the existing runner map, `maximum_concurrent` slot admission, systemd user-manager unit limits, owned unit cleanup and `execution.recover`.
- Add a pure core crate `kiln-microvm`:
  - machine admission bounds (vCPUs, memory against the unit memory limit, boot deadline, fixed source image size, warm pool slots);
  - a deterministic Firecracker VM plan (kernel, initrd, one read-only virtio-blk source drive, vsock, entropy, no network interface);
  - a lifecycle state machine that journals the job result before it destroys the VM;
  - a bounded framed guest protocol over vsock and result mapping;
  - a deterministic bounded ustar source archive.
- Add the guest agent binary `kiln-microvm-guest`. It is PID 1 in the guest. It extracts the source snapshot from the read-only drive into guest tmpfs and runs steps as an unprivileged user with no new privileges. Namespace creation and module loading are disabled in the guest.
- Add a hosted shell module that spawns `firecracker` inside the attempt's systemd unit, either from a config file (cold boot) or through the API socket (snapshot restore). It journals each job's VM result in SQLite before it stops the unit.
- Add an optional snapshot/restore warm pool with an explicit slot bound, per-slot locks and identity-checked snapshots.
- Add a `canonical_outcome` identity to every hosted execution receipt. It covers the backend-independent result: job and step states, exit codes and stdout.
- Add the x86_64-linux Nix flake output `microvm-guest`: the stripped nixpkgs kernel, an initramfs rootfs with the guest agent, the pinned `firecracker` binary and a manifest of exact paths and BLAKE3 identities.
- Extend the typed Nickel service contracts and fixtures for the new backend.
- Add a KVM-gated host test that runs the same pipeline on `namespaces` and `firecracker`, plus a guest-escape suite and a latency measurement.
- Document exactly which isolation claim this backend upgrades and which claims it does not make.

## Impact

- **Consumers:** hosted CI operators who choose a runner backend; customers see the same receipt shape plus `canonical_outcome` and a per-job `microvm` observation.
- **Owners:** Kiln hosted-composition maintainers own the shell and Nickel contracts; Kiln maintainers own `kiln-microvm` and the guest image.
- **Affected:**
  - `Cargo.toml` workspace members and `Cargo.lock`;
  - `crates/kiln-microvm` (new);
  - `crates/kiln-hosted` (execution, new `microvm` shell, tests);
  - `config/hosted-service.ncl`, `config/hosted-execution.ncl` and the hosted fixtures;
  - `flake.nix` packages;
  - `docs/hosted-ci.md` and `README.md`.

## Out of Scope

- No hostile multi-tenant or production isolation qualification. Receipts keep `hostile_isolation_claim: false`.
- No Firecracker jailer integration: it needs root, chroot and cgroup setup that this unprivileged user-manager deployment does not have. The docs say what it would add.
- No guest networking, no cache restore/publication for microVM runners, and no persistence of guest workspace writes back to the host.
- No change to the web-service internal developer loop (bubblewrap) or to the web-service worker-target inventory.
- No parallel matrix scheduling. Jobs stay sequential, as today.
