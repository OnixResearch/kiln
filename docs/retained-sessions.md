# Retained session lifecycle

`kiln-microvm::retained` is the pure ownership and lifecycle core for an opt-in
retained Lattice session. `kiln-hosted::microvm::retained` supplies host effects
through the existing Cloud Hypervisor runner, systemd unit planning, confinement
observations and owned cleanup; it is not a second VMM supervisor or placement
authority. The selected Aspen authority currently refuses retained effects.

A session binds a logical session identity, work/version, attempt/fence,
placement host and monotonic generation, and bounded CPU/memory/slot
reservations. `admit_retained_backend` checks only declared planning
capabilities: snapshot/restore, no guest network, seccomp/no-new-privs
confinement and x86_64. It does not qualify a live VMM or host. Every command carries an owner token with the exact session,
fence, host and generation; stale owners are refused.

The retained-session schema is `kiln.retained-session.v3`; checkpoint and bridge records retain their separately named `v2` schemas, and paused handoff records use `kiln.paused-handoff.v3`. Old session v2 records and old v1 records are not silently upgraded by inventing missing identities or fencing evidence.
Session, logical execution and VM identity are separate required fields; checkpoint profiles require independently matched initrd, device and actual memory-backing identities as well as image/kernel/VMM/CPU. Handoff records retain exact cutover evidence and recheck its binding before resume, including after replay.

The lifecycle admits receiving, restoring, paused, running, migrating,
migrated-away, retiring, reconciliation-required and retired states. Restore
observation fails closed into reconciliation without observed confinement.
Only a paused source may begin stopped migration. Migrated-away now requires
exact durable destination acknowledgement/placement and independently resolved
source volume/effect fencing receipts; a successful sender exit is insufficient.
`begin_retire` keeps capacity occupied in `retiring`. Only `observe_retired`
with affirmative cleanup observation releases it. Failed cleanup enters
reconciliation and can later accept a matching observed retirement.

## Requested versus observed controls

Pause and resume carry an exact operation ID and current session, attempt, fence and placement binding. Their bounded control record distinguishes requested, in-progress, observed-complete, rejected and uncertain outcomes. Persisting or sending a request never changes the observed `Running`/`Paused` state. Only a matching native VMM `vm.info` readback (and required host confinement/backend observation) adopts the target state; uncertainty remains occupied and requires reconciliation. Exact duplicate requests do not redispatch, and replies for another operation or owner are rejected. Pending controls block checkpoint, handoff, migration and retirement commands. Provider authority and external guest/stream/two-host qualification remain prerequisites; no local substitute is enabled.

## Paused handoff

`kiln-microvm::handoff` admits a checkpoint manifest binding the session,
work/version, attempt/fence, execution, source and target placement, operation,
image/kernel/hypervisor/CPU profile, exact disk generation and distinct
RAM/device-state/disk/workspace objects. A manifest is refused when its disk
identity or generation differs from the independently resolved volume facts, so
old memory cannot be paired with a newer disk.

`PausedHandoff` schema `kiln.paused-handoff.v3` orders intent, quiescence,
source pause, object verification, target admission, resume and source
retirement. Its serialized record retains the full originating
session/VM/work-revision/attempt/fence/execution identity, operation and
source/target placements. Every phase, reconciliation and retirement observation
compares the supplied session to that immutable origin; reopening does not
create a new origin. Old v2 records and records without the required origin
identity are refused rather than upgraded by inference.
Retirement and reconciliation observations validate the exact owner and origin
even after cleanup retires the source session. Ordered handoff commands still
require an active source; replaying an earlier intent cannot revive a retired
source.


Object verification compares the caller's independently resolved expectation
unchanged against both the origin and manifest. It never fills expected values
from the current session. Target admission, resume and source retirement
recheck the admitted manifest identity and exact operation, placement, target
session/fence, and cutover evidence including source volume/effect fencing and
durable destination acknowledgement. A `TargetAdmissionReceipt` must explicitly
pair its native reference with those scope facts; a syntactically valid
unrelated `ExternalRef` alone is insufficient. Refusal leaves phase/evidence
unchanged. These pure comparisons do not establish provider receipt truth or
effective distributed fencing. Object verification does not prove transfer or
durability, and target admission does not itself commit placement ownership.

## Runnable local consumer and host effects

`cargo run --locked -p kiln-hosted --example retained-host -- capabilities`
reports selected runtime support without claiming qualification. `profile CONFIG
BACKEND` verifies immutable VMM/kernel/initrd objects, runs the actual VMM
`--version`, checks it against the complete declared v54.0-family output, and
hashes actual stable CPU/microcode/kernel, CPU count, device and anonymous-private
memory-backing facts. Matching ISA or a pool label is not sufficient.
The returned profile contains both its canonical identity and the exact observed
facts. Before a retained host effect, the shell durably records those facts in
the private VM directory and refuses mismatches or incomplete prior evidence.

`apply EXISTING-DB CONFIG BACKEND OPERATION-JSON` reaches the retained hosted
consumer, but deliberately refuses before host mutation with the selected Aspen
provider. There is no local-lock or boolean override. A real implementation of
`RetainedAuthority::with_reservation` must atomically reserve exact current
attempt/fence/placement/capacity and enforce writer/effect exclusion around its
single-use callback. This is a producer prerequisite, not a Kiln-owned
distributed placement service.

The host shell implements cold creation, local paired restore, pause, resume,
snapshot and retirement using the existing reviewed CH backend. Detached
systemd units have `Restart=no`, private sockets and no guest NIC. Independent
confinement is observed before workload continuation, including after restore.
Cold guest boot is not Lattice workflow dispatch. No retained guest workflow or
reconnecting gateway is manufactured in Kiln.

The file-backed SQLite intent journal requires synchronous FULL/EXTRA and a
durable journal mode. Registration transactionally reserves the exact local
unit/directory/workspace and occupied host slot once. Every effect intent is
committed before mutation. A duplicate operation with changed bytes is refused;
an exact duplicate returns its prior observation or reconciliation without
redispatch. Pending, uncertain, migrated-away and retiring rows retain capacity.
A controller restart cannot authorize another launch merely because its process
map is empty. No source artifact deletion or automatic GC is performed.

The immutable registered binding also carries the original admitted absolute
wall deadline. A new/received unit gets only the remaining lifetime, with no CI
teardown grace added to the retained unit; pause and restore do not refresh it.
Resume checks the original deadline again. The authority provider must bind that
deadline to the original attempt rather than accepting a caller's extension.

`reopen_retired_checkpoint` (operator consumer: `reopen-checkpoint EXISTING-DB
OPERATION-JSON`) reacquires local capacity only for the exact same identity,
placement, resources, deadline and binding after observed retirement. It requires
the last durably paired paused checkpoint, rejects migrated-away sources, and
allows only restore, never a new cold dispatch. Every resume intent consumes
that checkpoint's restore authority before its host effect: unchanged workspace
bytes alone cannot justify rewinding old process memory after accepted effects.
Only the old VMM API/vsock socket inodes may be removed under the still-held
provider reservation and prior retirement observation; data and gateway sockets
remain. As with all retained host effects, the selected Aspen authority refuses
this operation until the real provider prerequisites exist.

## Paired local checkpoints and durable objects

Snapshots require an already observed paused VM and the provider's quiesced
volume/effect reservation. CH writes `config.json`, `state.json` and
`memory-ranges`; the shell syncs and hashes these together with the exact mutable
workspace and disk generation, then durably records
`kiln-paired-checkpoint.json`. Restore re-reads all objects and refuses changed
RAM/config/state/workspace, generation, identity, placement or profile. This is
local restore: cross-host snapshot path rewriting and distributed handoff are
not implicitly enabled by the descriptor.

`kiln-hosted::microvm::transfer::CheckpointStore` imports exactly sized BLAKE3
objects in 64 KiB buffers, syncs file and directory before a durable receipt,
and re-verifies all four distinct manifest objects before publication. Import
operation `.partial` evidence remains single-use after error or success;
incomplete/digest-mismatched copies cannot publish. Socket callers must enforce
a read timeout as well as the checked cancellation/deadline.

The operator consumer supports `import-object PRIVATE-STORE OPERATION-HEX
OBJECT-JSON INPUT` and `verify-object PRIVATE-STORE OBJECT-JSON`. `OBJECT-JSON`
has exact raw-hex `identity` and `bytes`. The store must be a service-owned
private 0700 directory. Local import is not a cross-host Iroh qualification or
ownership commit.

`publish-checkpoint PRIVATE-STORE MANIFEST-JSON INDEPENDENT-FACTS-JSON
OBJECTS-JSON` reaches the actual manifest publication path. The facts must be
resolved independently of the proposed manifest; the four exact object receipts
are supplied in RAM/device/disk/workspace order. Publication re-reads every
object and checks the complete v2 scope/profile against those facts. Its durable
receipt explicitly grants neither placement commit nor destination resume.

## Drain and required qualification

`retained::drain` keeps completed, migrated, checkpointed, finishing, pinned and
unknown dispositions separate. Migration/retiring/cleanup uncertainty blocks a
drained claim. Durable continuation without observed source retirement is not
drained; empty scheduler metadata is not retirement evidence. The pure cost and
cooldown guard rejects future/unknown move times, moves whose expected cost
consumes the remaining work, and repeated moves within cooldown. It grants no
move or retry authority.

After migration dispatch, ordinary nonzero sender exits, source Running, lost
final ACK, or a currently stopped target that previously wrote all retain both
sides for reconciliation unless exact durable placement/ACK and enforced fences
are independently resolved. They never authorize stopping the target or blindly
resuming stale source memory.

`retained-host require-migration` exits nonzero for the selected unsupported
profile. In the real host harness, set
`KILN_MICROVM_MIGRATION_REQUIRED=1` and run
`cargo test --locked -p kiln-hosted --test microvm
retained_migration_required_cannot_skip_unqualified_provider -- --nocapture`.
This required gate fails rather than skips while the selected provider/campaign
is unavailable. Its ordinary negative admission test is not a migration success.
No two-host migration or hardware campaign was run for this change.

## Lattice worker guest (single host)

`nix build path:$PWD#lattice-worker-guest` (x86_64-linux) builds the pinned
Lattice worker guest for Kiln #51. It contains:

- the reviewed guest kernel shared with the per-job images;
- an immutable initramfs with Lattice
  `661499d282c2e79ebefbd0f9b35beb5f8d7795cd`, the admitted `log-triage`
  (Lattice example) and `workspace-journal` (Kiln fixture) workflows, busybox and
  the `kiln-lattice-worker-guest` shell;
- Cloud Hypervisor 54.0 from the guest nixpkgs pin;
- `manifest.json`.

Lattice comes from the separately locked
`runtime/lattice-worker-cohort` subflake (see
[reference pins](reference-pins.md#lattice-worker-guest-cohort)).

The manifest has schema `kiln.lattice-worker-guest.v1`. It binds:

- the kernel, initramfs and VMM paths with their BLAKE3 identities;
- the exact `cloud-hypervisor --version` output;
- the embedded `kiln.lattice-worker-environment.v1` record and its identity;
- a raw workspace disk, vsock port 10241 and no NIC;
- 1 vCPU, 768 MiB and a 64 MiB workspace.

The environment record binds the Lattice revision and binary, the guest shell
and busybox, and, for each workflow, its definition bytes. It also records the
exact effect requirements that the pinned Lattice binary reported in the build
sandbox.

`kiln_microvm::lattice_worker` is pure. `admit_attempt` (host) and
`admit_dispatch` (guest) refuse a request whose schema, image, environment,
workflow, definition, workflow revision, workspace identity or generation, or
guest identity differs, or whose deadline is spent. The workspace identity and
the 32-hex guest machine identity are BLAKE3 key derivations over the
attempt, the execution and the workspace generation. `authority_profile`
renders Lattice's standalone effect-authority profile from the baked
requirements, with the original attempt deadline as its expiry.

`kiln_hosted::microvm::lattice_worker` is the host shell:

- `load_manifest` re-hashes every object.
- `backend` returns the existing `cloud-hypervisor` runner with the declared
  version.
- `create_workspace` creates a sparse 0600 raw image once.
- `dispatch` sends at most one dispatch, and only to a retained session whose
  record is `Running`. It commits `hosted_lattice_worker_dispatch` before
  sending, accepts only the confined VMM PID on the 0600 `vsock.sock_10241`
  listener, and records the guest report. A repeat, or a restarted controller,
  returns the recorded report, or `ReconcileRequired` if none was recorded. It
  never resends.
- `observe` reports the attempt that a running incarnation carries, for
  example after restore.

The guest refuses a workspace image whose first 64 KiB were ever written. It
formats a blank image as ext2, writes a root-owned label, Lattice config and
authority profile, and runs `lattice import`, `lattice authority inspect`
(which must equal the baked requirements) and `lattice run --output json` as
uid 1000 under the original deadline. It persists `/kiln/report.json` in the
workspace and accepts no second dispatch in that incarnation.

Single-host KVM qualification:

```console
nix build path:$PWD#lattice-worker-guest --out-link /tmp/klw-guest
PATH=$(nix build --no-link --print-out-paths nixpkgs#e2fsprogs.bin)/bin:$PATH \
KILN_MICROVM_REQUIRED=1 KILN_LATTICE_WORKER_GUEST=/tmp/klw-guest \
  cargo test --locked -p kiln-hosted --test lattice_worker -- --nocapture
```

Without `/dev/kvm`, the guest, the systemd user manager or `debugfs`, the test
prints `SKIP`. It fails instead when `KILN_MICROVM_REQUIRED=1` is set.

The test drives the existing retained shell through:

- host refusal of crossed attempts before any effect;
- two concurrent cold boots that pause and then run;
- guest refusal of a crossed dispatch;
- `log-triage` and `workspace-journal` runs with distinct guest identities;
- stale-generation and foreign-host refusal;
- controller restart without redispatch;
- pause, paired snapshot, retirement with released capacity, and refusal of a
  cold start after retirement;
- `debugfs` readback of each workspace;
- checkpoint reopen, restore and resume, with the same attempt observed in
  guest memory;
- refusal of a workspace that attempt A already used.

The test supplies a test-only `RetainedAuthority` that only checks the single
host name. That is not ownership or fencing. The production consumer still
refuses through the selected Aspen authority.

## Non-claims

Pure modules do not establish distributed ownership. Hosted mechanisms are not
enabled with the selected provider: Aspen current-placement/resource fencing,
admitted cross-host streaming, Lattice's retained guest execution/reconnect
gateway, and a compatible two-host CH/final-ACK qualification remain required.
The local host has `/dev/kvm`; this observation is not evidence of a compatible
second host or safe resource fencing. No service activation, host-policy change,
live-migration enablement or production qualification is claimed. Maintenance
owner is the Kiln hosted/microvm adapter; Aspen and Lattice retain their provider
responsibilities.
