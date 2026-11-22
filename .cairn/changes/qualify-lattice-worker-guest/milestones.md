# Milestone plan for Kiln #51–#55

Issues were read on 2026-11-19 at Kiln `3cdb0ce3`. Lattice #9–#12 were read on the same day. Evidence labels:

- **KVM**: real Cloud Hypervisor 54.0 on this one KVM host.
- **fixture**: pure or injected-adapter tests.
- **unqualified**: not exercised.

Prerequisite tags:

- `multi-host`: two compatible KVM hosts.
- `onix-core`: onix-core or onixos service, module or inventory changes.
- `aspen`: Aspen #15 cross-host streaming, plus a placement and resource-fencing provider.
- `lattice`: Lattice #10, #11 or #12 producer work.
- `infra`: shared storage, relays or other infrastructure that does not exist here.

## Milestones

| Milestone | Scope | Qualification here |
|---|---|---|
| M1 (this change) | #51 Kiln-side guest packaging and manifest, attempt admission and single dispatch. #52 single-host retained lifecycle on the existing shell: cold boot, pause, resume, snapshot, retire, restore, stale-owner refusal, restart without redispatch | KVM plus fixture |
| M2 | Admitted placement and fencing provider (local or Aspen). onix-core service integration: systemd ownership gate, GC roots, scoped credentials, static guests. Lattice #10 profile and #11 guest gateway inside this image | Needs `onix-core`, `aspen`, `lattice` and a parent decision |
| M3 | #53 paired workspace and state transfer, paused A→B and B→A relocation | Needs `multi-host`, `aspen`, M2 |
| M4 | #54 Iroh and Aspen/Molten control, log and migration channels | Needs `multi-host`, `aspen`, `lattice` (#11), `infra` (relay policy) |
| M5 | #55 live precopy, compatibility matrix and host draining | Needs `multi-host`, `aspen`, M3, M4, and an upstream CH #9208 recheck |

## #51 Package pinned Lattice worker guests

| Acceptance criterion | Milestone | Status after M1 |
|---|---|---|
| Nix builds a guest and manifest with exact identities | M1 | KVM. `nix build .#lattice-worker-guest`; the manifest is re-verified before use |
| One multi-step workflow through Lattice's production executor in the guest | M1 (headless `lattice run`); M2 (#10 envelope, workflow exchange) | KVM for the headless executor. The `lattice` #10 attempt envelope is unqualified |
| Two attempts: separate mutable state and identities, shared immutable artifacts | M1 | KVM |
| Wrong image, workflow, environment or workspace rejected before dispatch | M1 | fixture plus KVM: guest-side refusal of a crossed dispatch |
| Host and guest GC keep closures for active attempts and checkpoints | M2 `onix-core` | Unqualified. The M1 guest has no Nix store or daemon, so guest GC does not apply. Host GC roots need the onix-core service |
| Disk CLI/API format against the pinned VMM, cold boot and restore | M1 | KVM: `image_type: Raw` on CH 54.0, cold boot plus paired restore |
| Guest filesystem and credential boundary; no-network CI unchanged | M1 partial; M2 `onix-core` | M1: no NIC, no host share, per-attempt authority profile only. Scoped model or provider credentials are unqualified |
| Explicit failures without KVM; scripted versus KVM evidence separate | M1 | KVM plus fixture |
| Static CH guests keep their behaviour | M2 `onix-core` | Unqualified; cross-repo |
| Iroh/Aspen host adapter and guest bridge packaged | M4 `aspen` `lattice` | Unqualified |
| Host identity keys outside guest images | M2/M4 `onix-core` | M1: the image contains no key material. Host key provisioning is unqualified |
| Protect per-VM CH sockets and bind to admitted VM and placement | M1 partial; M2 | KVM: the private 0600 API socket and peer-PID check are exercised. Placement binding needs a provider |
| Cross-host services through Aspen #15 | M4 `multi-host` `aspen` | Unqualified |
| CH binary identity together with launch config; both hosts match | M1 one host; M3 `multi-host` | KVM on one host only |
| Memory backing and mergeability for boot, migration and restore | M1 boot and restore; M3/M5 migration | KVM: anonymous-private, `shared=false`, `mergeable=false` |
| Immutable closure staged on both hosts | M3 `multi-host` | Unqualified |

## #52 Owned relocatable sessions

| Acceptance criterion | Milestone | Status after M1 |
|---|---|---|
| Session on the existing CH runner survives controller restart without duplicate dispatch | M1 | KVM on one host: journal reopen, repeated operation and repeated dispatch |
| Concurrent placement and resume authorize one owner; duplicates reconcile | M1 duplicates; M2 ownership `aspen` | KVM for duplicates. Concurrent ownership is unqualified |
| Obsolete placement cannot write its volume, register a candidate or gain effect authority | M2/M3 `aspen` | Unqualified; no fencing provider |
| Source reboot or systemd restart after migration cannot start the old VM | M1 single host; M3 | KVM: `Restart=no`, and cold start after retirement is refused. After migration: unqualified |
| Host loss and uncertain cleanup keep unknown state and capacity | existing fixture; M3 host loss | fixture |
| Every received or restored spawn is checked for confinement first | M1 | KVM restore path |
| Per-job cold/warm and network-denial fixtures unchanged | M1 | Unchanged code; existing tests |
| VMM version checked against actual CLI/API plans, including `image_type` | M1 | KVM |
| Faults: ownership update loss, stale commands, controller restart, cleanup failure | existing fixture; M1 stale and restart | fixture plus KVM for stale command and restart |
| Iroh/Molten host control (four items) | M4 `aspen` `multi-host` | Unqualified |
| Requested, in-progress, observed, rejected and uncertain controls | existing fixture; M1 | KVM pause and resume readback |
| Retirement cannot free capacity before observed cleanup | existing fixture; M1 | KVM retirement; fixture for lost ACK |
| Serialized mutations; handles held across cancellation | existing fixture; M2 | fixture |
| Races: pause+capture, resume/cancel, migrate/retire, duplicates, restart | existing fixture; M1 pause+capture | fixture plus KVM pause then snapshot |
| Restored devices and confinement ready before guest execution | M1 | KVM: confinement after restore, guest reconnect |
| Native evidence separate from the pure response; premature-completion control | existing fixture | fixture |
| Exact identities retained; missing prerequisites explicit | M1 | KVM plus fixture |

## #53 Workspace transfer and paused migration

Every acceptance criterion needs `multi-host` and `aspen`, and the volume provider from M2, so all belong to M3. Local fixtures already cover incomplete manifests, old memory with a changed disk generation, and ordinary nonzero sender exits. M1 adds one real local paired snapshot and restore on one host. That is not relocation.

## #54 Iroh and Aspen/Molten channels

Every acceptance criterion belongs to M4 and needs `multi-host`, `aspen`, `lattice` #11 and `infra` relay policy. M1 keeps the no-device boundary: the guest has vsock only, and the host API socket stays host-private. Static guest and Redox tap0 fixtures belong to `onix-core`.

## #55 Live migration and host draining

Every acceptance criterion, including the Propolis directional matrix and the NixVegas progression, belongs to M5 and needs `multi-host`, `aspen`, M3 and M4. M5 also needs a recheck of upstream CH #9208. The existing `retained_migration_required_cannot_skip_unqualified_provider` gate still fails when required. M1 leaves existing cold/warm per-job behaviour and confinement gates unchanged.
