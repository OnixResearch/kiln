## Context

`kiln-microvm::retained` and `kiln-hosted::microvm::retained` already own retained lifecycle decisions and Cloud Hypervisor host effects. These cover the private API socket, `Restart=no` systemd units, confinement observation, the durable intent journal, paired checkpoints and observed retirement. The per-job CH guest, `microvm-guest-ch`, is a busybox initramfs with the Kiln guest agent. Its reviewed kernel and VMM come from the `guest-nixpkgs` pin: Linux 6.18.60 and Cloud Hypervisor 54.0. Lattice `661499d2` provides `lattice import`, `lattice authority inspect` and the headless `lattice run`, configured by a Nickel application config and a standalone effect-authority profile. The full milestone map is in `milestones.md`.

## Goals and Success Contract

M1 is single-host only:

- A Nix-built guest manifest whose identities the host re-verifies.
- Attempt admission that refuses crossed identities on both sides of the vsock boundary before any effect.
- Two attempts with distinct mutable state and identities over shared immutable artifacts.
- One multi-step workflow run by Lattice's production headless executor inside the guest.
- A real-KVM pass through the existing retained shell: cold boot, resume, dispatch, pause, snapshot, retire, reopen, restore and resume.
- Dispatch at most once across a controller restart.

Everything else in #51–#55 stays labelled unqualified.

## Decisions

### Decision: pin Lattice through a content-addressed cohort subflake

**Choice:** `runtime/lattice-worker-cohort/flake.nix` has a single locked input, Lattice `661499d2`, with its own lock file. The root flake reaches it lazily through `builtins.getFlake` with a NAR hash, the same pattern as `runtime/build-cohort`. Only the x86_64-linux `lattice-worker-guest` package uses it.

**Rejected:**

- A root flake input would pull Lattice's whole input graph into Kiln's root lock and move it on every root update.
- Building Lattice inside Kiln would create a second build owner for Lattice.

### Decision: split the environment record from the image manifest

**Choice:**

- The initramfs carries `/etc/kiln/lattice-worker-environment.json`, schema `kiln.lattice-worker-environment.v1`. It records the Lattice revision and binary BLAKE3, the guest shell and busybox BLAKE3, and one entry per admitted workflow. Each workflow entry records the definition BLAKE3 and the exact `lattice.standalone-effect-authority.v1` requirements (`workflow_revision`, `machine_identity`, `policy_ref` and the grants). The pinned Lattice binary computes these requirements in the Nix sandbox at image build time.
- The outer manifest, schema `kiln.lattice-worker-guest.v1`, binds the architecture, the kernel, initramfs and VMM objects (path and BLAKE3), the actual `cloud-hypervisor --version` output, the environment record and its identity, the device profile and the resource limits.
- The environment identity is BLAKE3 over the canonical environment JSON. The image identity is BLAKE3 over the canonical manifest JSON.

**Rationale:** an initramfs cannot contain its own hash. Baking the requirements lets the host render each attempt's authority profile without running Lattice on the host, and lets the guest check that its workflow is exactly the one admitted.

### Decision: pure attempt admission on both sides

**Choice:** `kiln_microvm::lattice_worker` is pure: no I/O, clock or process access.

- `admit_attempt(manifest, request, now_ms)` runs on the host. It checks the schema, image identity, environment identity, workflow name, definition and revision, workspace identity and generation, guest identity derivation and the remaining deadline.
- `admit_dispatch(environment, dispatch, now_ms)` repeats the environment, workflow and identity checks in the guest against the baked record.

Both return a typed `Refusal` before any effect.

The per-attempt guest identity is BLAKE3 `derive_key` over the attempt, execution and workspace identities, truncated to a 32-hex machine-id. `authority_profile` renders the Nickel effect-authority profile from the baked requirements and the attempt's original deadline. It does not take a caller-supplied extension.

### Decision: blank per-attempt workspace image, formatted by the guest

**Choice:**

- The host creates a sparse raw image with `create_new` and mode 0600 at the retained binding's workspace path.
- The guest refuses any image that is not blank in the probed superblock region. It formats a blank image with busybox `mke2fs`, mounts it with the `ext4` module, and writes a workspace label. Lattice's `data_dir`, the attempt's checkout directory and its result live there.

**Rationale:** no host mount or root is needed. A disk that another attempt already used is refused. Immutable content stays in the store-backed initramfs.

### Decision: one dispatch per attempt, journalled before it is sent

**Choice:**

- The guest shell connects to the host on vsock port `10241`. This port is distinct from the per-job agent port, so a per-job host never talks to a worker guest.
- The host side binds `vsock.sock_10241` with mode 0600 in the retained VM directory. It accepts only a peer whose `SO_PEERCRED` PID is the confined VMM PID.
- The hosted shell requires the retained record to be `Running`. It commits `hosted_lattice_worker_dispatch(attempt, session, request, outcome NULL)` before sending.
- A repeated or restarted dispatch returns the recorded report, or `ReconcileRequired` if no report was recorded. It never resends.
- The guest accepts at most one dispatch per incarnation and reports its attempt on later pings. A restored guest therefore shows the continuation of the same attempt.

### Decision: test-only single-host reservation

**Choice:** The real-KVM test implements `RetainedAuthority` in the test file. The implementation checks that the operation's placement host equals the test host. `SelectedAspenAuthority` and the `retained-host` consumer still refuse.

**Rejected:** a production host-local lock provider. The archived retained-mobility design forbids degrading to a local lock. Changing that needs a parent decision and belongs to M2.

### Deterministic ordering and failure behaviour

1. Manifest verification.
2. Host admission.
3. Workspace creation.
4. Registration.
5. Cold start.
6. Resume.
7. Dispatch journal.
8. Send.
9. Report.

A failure before step 7 has no dispatch effect. A send or receive failure after step 7 returns `ReconcileRequired` and is never retried. Guest refusal leaves the guest undispatched and reports the refusal.

## Tests

- Pure tests: positive admission, each crossed identity, an expired deadline, distinct attempt identities, profile rendering and frame bounds.
- Hosted tests: manifest re-verification with corrupted objects, workspace creation refusal on existing paths, and the dispatch journal with a duplicate, a crash after the intent and a non-Running session.
- KVM test, `crates/kiln-hosted/tests/lattice_worker.rs`: the lifecycle scenario plus negatives. It skips explicitly, or fails under `KILN_MICROVM_REQUIRED=1`.

## Risks and Trade-offs

- The initramfs carries the Lattice closure, about 100 MiB uncompressed, so each guest needs 512 MiB.
- The ext2 workspace has no journal. Durability rests on the guest's explicit `sync` before reporting, and on the paired snapshot taken while paused.
- A host-local test reservation is not ownership. This is stated in the test and the evidence.

## Claim Boundaries

M1 does not establish:

- placement authority, volume or effect fencing;
- cross-host relocation, Iroh transport, live migration or draining;
- onix-core service integration or GC-root retention;
- Lattice #10 attempt-envelope or #11 gateway semantics;
- a production or hostile-isolation qualification.

The KVM receipt binds one host, one CH build and one guest manifest.

## Verification Strategy

Focused `cargo test -p kiln-microvm -p kiln-hosted`, then the KVM test with `KILN_MICROVM_REQUIRED=1`, then the repository gates: workspace tests, clippy, fmt, the Nickel fixtures, Cairn validate/traceability/readiness, the octet build and `nix flake check`.
