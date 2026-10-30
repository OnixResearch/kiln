## Context

`crates/kiln-hosted/src/execution.rs` owns hosted execution:
- `Config.runners` maps a runner label to a `Backend`;
- `check_capacity` admits at most `maximum_concurrent` reserved, running or unknown attempts (the slot model);
- `run_pipeline` runs ready jobs sequentially;
- `run_step` launches one step under `systemd-run --user` in the attempt unit `kiln-hosted-<identity>.service`, with `MemoryMax`, `TasksMax`, `CPUQuota` and `RuntimeMaxSec`, and `cleanup_unit` proves that the unit's cgroup is empty.

`Backend::Namespaces` is trusted-only. `Backend::Qemu` boots one VM per step and maps the workspace over 9p. Executables and images are `{path, blake3}` inside `/nix/store` and are verified before use. Receipts always report `hostile_isolation_claim: false`. Operator configuration is typed Nickel (`config/hosted-service.ncl`, `config/hosted-execution.ncl`) and exported to JSON.

Host facts: `/dev/kvm` and `/dev/vhost-vsock` are usable by the service user. The flake-pinned nixpkgs provides Firecracker 1.15.1 and kernel 6.18.42, whose virtio-mmio, virtio-blk and vsock drivers are modules.

## Goals and Success Contract

A runner label mapped to `kind = "firecracker"` executes each job in its own Firecracker microVM, inside the same slot, unit-limit, cleanup and recovery model as the other backends. For the same reviewed pipeline and source, it produces the same `canonical_outcome` identity as `kind = "namespaces"`.

Inside the guest, a job cannot:
- create namespaces;
- see host processes, host `/proc` or host `/sys`;
- reach any network.

A VM is destroyed only after its job result is journaled. Uncertain teardown retains the slot. Cold-start and snapshot-restore latencies are measured on this host.

False completion includes:
- a VM plan with a network interface or a writable source drive;
- destroying a VM before its result row is written;
- a warm snapshot reused after its firecracker, kernel, initrd or machine identity changed;
- an escape probe that "passes" because the probe tool is missing;
- a host test that reports a VM result without KVM, or that cannot be forced to fail when KVM is required;
- docs or receipts that claim hostile multi-tenant qualification.

## Decisions

### Decision: Register as a hosted runner backend

**Choice:** add `Backend::Firecracker` next to `Namespaces` and `Qemu`. The job's runner label selects it.
- Firecracker runs inside the attempt unit with the same systemd limits.
- `maximum_concurrent` is the slot bound: an attempt holds one slot, and its jobs run one VM at a time.
- `execution.recover` keeps working unchanged because the VM lives in the attempt unit.
- Hostile trust is admitted only with an operator `qualification_identity`, the same rule as `qemu`. Receipts still state `hostile_isolation_claim: false`.

The web-service worker-target inventory describes Lattice workflow bindings, not hosted runners, so it is unchanged.

### Decision: Pure core crate `kiln-microvm`

**Choice:** a new crate with no I/O in its library:
- `admit_machine(policy, host)`: vCPUs 1..=8; memory 128..=16384 MiB; guest memory plus a named Firecracker overhead must fit the unit `MemoryMax`; vCPUs plus a named thread allowance must fit `TasksMax`; boot deadline 100..=60000 ms; source image 1..=1024 MiB in whole MiB; warm slots 1..=16 and at most `maximum_concurrent`.
- `vm_config(plan)`: the Firecracker config-file JSON. It has a kernel, an initramfs, one drive (`is_read_only: true`, not the root device), vsock with a fixed guest CID, an entropy device and no `network-interfaces`.
- `snapshot_identity(...)`: a BLAKE3 identity over the Firecracker/kernel/initrd hashes, the machine policy, the boot arguments and the slot.
- `Lifecycle`: `Planned → Starting → Ready → Running → Collected → Journaled → Destroying → Destroyed | Abandoned`. Failures go to `Collected` with partial results. `Destroy` is emitted only from `Journaled`, or after `JournalFailed`. Every other transition is rejected.
- Guest protocol: 4-byte big-endian length plus JSON frames, at most 4 MiB. Host messages are `Ping` and `Run`; guest messages are `Pong`, `Step` and `Done`. The schema is checked on every frame.
- `guest_environment` and `shell_arguments` reproduce the namespace backend's environment and shell flags exactly.
- `source_archive(entries)`: deterministic ustar with sorted paths, zero mtime/uid/gid, normalized modes, symlinks kept and named entry/byte bounds.

### Decision: Source snapshot over a read-only virtio-blk image

**Choice:**
- The shell walks the scoped workspace without following symlinks and builds the bounded archive. It writes the archive to a private image file sized exactly `source_image_bytes`.
- The guest flushes the block cache, extracts the drive into a guest tmpfs `/work` owned by the job user, and discards all writes with the VM.
- Steps in one job share `/work`, as they do in the namespace backend. Jobs do not share guest writes.
- Cache policies naming a Firecracker runner are rejected at configuration time, because publication would read the unchanged host workspace.

### Decision: One host vsock listener per VM session

**Choice:** before it starts or restores a VM, the host binds one Unix listener at Firecracker's hybrid vsock path `<uds_path>_<port>`. The guest agent connects to host CID 2 on that fixed port and retries until it succeeds. The host accepts exactly one connection, then removes the listener. The first framed `Pong` or `Step`/`Done` exchange marks the guest ready.

This works the same after cold boot and after snapshot restore. A restore resets established connections, and the agent simply reconnects. Once the listener is removed, a job's guest-initiated vsock connection to any host port is refused. The guest side only calls `connect`; it never needs `accept`, which `nix` exposes only as a raw descriptor and which would need `unsafe`, forbidden by the workspace lints.

### Decision: CET-free CPU template

**Choice:** every VM, cold or restored, uses a Firecracker custom CPU template that clears CPUID CET_SS/CET_IBT and the CET_U/CET_S supervisor XSAVE bits.

On this host (AMD Zen 5, host kernel 6.18 with KVM CET virtualization), Firecracker 1.15 restores a snapshot without the guest's CET supervisor state. The restored guest then faults in `XRSTORS` (`restore_fpregs_from_fpstate`). With the template, restore works. Cold boots use the same template, so both boot modes expose identical CPUs. The cost is that guest user programs cannot use shadow stacks.

### Decision: Guest image from the pinned nixpkgs

**Choice:** a flake package `microvm-guest` containing:
- the nixpkgs kernel `vmlinux`, stripped of debug info;
- a `makeInitrdNG` initramfs (the NixOS initrd builder) with busybox, bash, the guest agent and a decompressed `makeModulesClosure` module set (virtio_mmio, virtio_blk, vsock transport, virtio_rng, vmgenid);
- `manifest.json` with exact store paths.

The init script:
- loads the modules, then sets `kernel.modules_disabled=1`;
- sets every `user.max_*_namespaces` to 0, `dmesg_restrict=1` and `kptr_restrict=2`;
- mounts tmpfs `/work` and `/tmp`;
- execs the agent as PID 1.

The agent sets `no_new_privs` and runs each step as uid/gid 1000 in its own process group, with bounded output and a deadline.

This is a Nix-built initramfs over the NixOS kernel, not a full NixOS system closure.

### Decision: Optional warm pool by snapshot/restore

**Choice:** `warm_pool = { directory, slots }`.
- Each slot has a private directory, an exclusive `flock`, and a snapshot made with that slot's fixed drive and vsock paths. Restore needs exactly those paths.
- A missing or stale snapshot (identity or state hash mismatch) is rebuilt: cold boot with a blank source image, `Ping`, pause, full snapshot, stop.
- A job restores with `resume_vm`, after its source image has replaced the slot's drive file.
- The guest drops cached blocks before reading the drive. The host sends wall-clock time and a fresh 32-byte seed, which the guest mixes into `/dev/urandom`; VMGenID also notifies the guest kernel.
- When all slots are busy, the job cold boots and records `warm_pool: exhausted`.

### Decision: Journal before destroy

**Choice:** a `hosted_microvm_job` row is written as `starting` before spawn. Collected step results are written as `journaled`. Only then is the unit stopped. The row ends as `destroyed` or `destroy_uncertain`. An uncertain teardown makes the attempt `unknown`, so it keeps its slot until `execution.recover`.

### Decision: Canonical outcome

**Choice:** `canonical_outcome(receipt)` keeps the pipeline id, the overall state and, per job, the job id, state and steps (name, state, exit code, stdout, stdout truncation).

It excludes elapsed times, cleanup flags, stderr, signals and backend observations. It is the BLAKE3 identity of that projection and is added to every receipt. Stderr is excluded because the namespace launcher (`systemd-run`) writes its own unit diagnostics there.

### Decision: Jailer not integrated

**Choice:** the shell runs Firecracker unprivileged inside a systemd user-manager unit and relies on Firecracker's built-in seccomp filters. The jailer needs root to chroot, create cgroups and switch uid, so no `jailer` field is accepted. The docs explain what the jailer would add: a chroot, a dedicated uid per VM, network namespace separation and a cgroup per VM.

## Risks / Trade-offs

- The stock nixpkgs kernel is a general distro kernel, so a cold boot takes on the order of a second; a microVM-tuned kernel would be faster. The warm pool removes most of that cost. Latencies are measured and recorded rather than assumed.
- One snapshot serves many jobs. Guest memory state, including kernel RNG state from before the restore, is shared between restores. The host seed and VMGenID reseed mitigate this; the risk is documented.
- Snapshot files are trusted because they live in a private 0700 directory owned by the service user. Only the state file is hashed, to keep restore latency low.
- Guest workspace writes do not persist to the host, so cache publication is rejected for Firecracker runners.

## Verification Strategy

- `kiln-microvm` unit tests, positive and negative: admission bounds, VM plan invariants, lifecycle transitions (including illegal ones and journal failure), frame bounds and schema rejection, environment and argument parity, archive determinism and bounds.
- `kiln-hosted` unit tests: Firecracker config validation (cache rejection, unsafe pool directory), hostile admission, and the pure-core canonical equivalence between namespace and microVM step observations.
- Nickel: the positive fixture with a Firecracker runner, plus negative fixtures for a network field and an oversized pool.
- A KVM-gated host test (`crates/kiln-hosted/tests/microvm.rs`) using the built `microvm-guest`:
  - the same pipeline on both backends gives equal canonical identities;
  - the guest-escape suite is denied;
  - cold-start and restore latencies are measured.
  Without KVM or artifacts, it prints an explicit skip reason; `KILN_MICROVM_REQUIRED=1` turns that into a failure.
- Evidence (`evidence/verification.json`) records commands, numbers and probe results.
