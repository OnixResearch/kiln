# Hosted Microvm Execution Specification

## Purpose

Defines the `hosted-microvm-execution` capability.

## Requirements

### Requirement: Firecracker backend admission

r[kiln.hosted.microvm.admission]
A hosted runner of kind `firecracker` MUST be admitted only with immutable Nix-store `firecracker`, kernel and initramfs identities, and with named finite bounds:
- vCPUs and memory that fit the unit's `TasksMax` and `MemoryMax`;
- a boot deadline;
- a fixed source image size;
- warm pool slots no greater than `maximum_concurrent`.

It MUST register through the existing runner map, slot capacity, unit limits and owned cleanup. Configuration that names a Firecracker runner in a cache policy, or that has unknown fields, MUST fail before any effect.

#### Scenario: Out-of-bounds machine

r[kiln.hosted.microvm.admission.scenario.bounds]
- GIVEN a Firecracker runner whose memory exceeds the unit limit, whose vCPUs or warm slots exceed their bounds, or that is used by a cache policy
- WHEN the execution configuration is admitted
- THEN admission MUST fail and no VM or unit MUST start

### Requirement: Deterministic isolated VM plan

r[kiln.hosted.microvm.plan]
The pure VM plan MUST be deterministic. It MUST contain exactly one virtio-blk drive, which is the read-only, non-root source snapshot image, plus one vsock device, one entropy device, and no network interface. The source snapshot MUST be a deterministic bounded archive of the scoped customer workspace.

### Requirement: Journal-before-destroy lifecycle

r[kiln.hosted.microvm.lifecycle]
Each job MUST run in its own microVM. The lifecycle state machine MUST emit the destroy effect only after the job result has been journaled, or after journaling has failed. Uncertain teardown MUST leave the attempt unknown, so it keeps its capacity slot.

#### Scenario: Result journaled before destruction

r[kiln.hosted.microvm.lifecycle.scenario.journal]
- GIVEN a microVM whose guest has returned step results, or that has failed
- WHEN the shell follows the lifecycle effects
- THEN the result row MUST be written before the unit is stopped
- AND a destroy request before collection MUST be rejected as an illegal transition

### Requirement: Bounded guest protocol

r[kiln.hosted.microvm.protocol]
Host and guest MUST exchange length-prefixed JSON frames over vsock, bounded by a named maximum and checked against a versioned schema. Steps MUST run in the guest as an unprivileged user with no new privileges, with the namespace backend's environment and shell flags, a deadline and bounded output.

### Requirement: Backend-equivalent canonical outcome

r[kiln.hosted.microvm.equivalence]
Every hosted execution receipt MUST carry a `canonical_outcome` identity over the backend-independent result. The same reviewed pipeline over the same source MUST produce the same `canonical_outcome` on the `namespaces` and `firecracker` backends.

#### Scenario: Same pipeline on both backends

r[kiln.hosted.microvm.equivalence.scenario.host]
- GIVEN KVM, the built guest image and a reviewed pipeline with succeeding and failing steps
- WHEN it runs once on a namespace runner and once on a Firecracker runner
- THEN both receipts MUST report the same `canonical_outcome`
- AND a host without KVM or the guest image MUST print an explicit skip reason instead of a VM result, and MUST fail when `KILN_MICROVM_REQUIRED` is set

### Requirement: Guest escape denial

r[kiln.hosted.microvm.escape]
Inside the microVM, a job MUST NOT be able to:
- create user, mount, network or PID namespaces;
- observe host processes or host-identifying `/proc` and `/sys` state;
- reach a network, whether over IP or guest-initiated vsock.

Each probe outcome MUST be recorded.

#### Scenario: Escape suite

r[kiln.hosted.microvm.escape.scenario.suite]
- GIVEN a running microVM job
- WHEN it attempts namespace creation, host `/proc` and `/sys` probing, and network egress
- THEN every attempt MUST be denied, with the probe tooling shown to be present

### Requirement: Bounded warm pool

r[kiln.hosted.microvm.warm_pool]
An optional warm pool MUST be bounded by an explicit slot count. Each slot MUST hold an exclusive lock and an identity-bound snapshot, which MUST be rebuilt when its identity or state hash changes. A restored VM MUST receive fresh time and entropy, and it MUST still be destroyed per job. Cold-start and restore latency MUST be recorded.

### Requirement: Explicit isolation claims

r[kiln.hosted.microvm.claims]
Documentation MUST state which isolation claim the Firecracker backend upgrades (a separate guest kernel behind KVM instead of the host kernel's namespaces) and which claims it does not make (hostile multi-tenant or production qualification, the jailer, side channels, host kernel KVM bugs). Receipts MUST keep `hostile_isolation_claim: false`.
