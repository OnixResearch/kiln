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

### Requirement: Read-only Firecracker cache drive

r[kiln.hosted.microvm.cache_drive] A Firecracker cache policy MUST materialize
the restored cache entry as a read-only second virtio-blk drive and the guest
MUST extract it over `/work` after the source drive. The drive MUST NOT be
writable and MUST NOT be the source drive. After a successful job and confirmed cleanup, the host MUST request the
declared output roots, bound and validate the returned ustar, extract it
without traversal, and publish through the cache store. Publication MUST NOT
occur after failure or uncertain teardown.

#### Scenario: Cache image is read-only

- GIVEN a Firecracker runner with a cache policy and a hit
- WHEN the VM configuration is built
- THEN it has a second read-only `cache` drive.

#### Scenario: Publish guest outputs and restore them

- GIVEN a Firecracker runner with a cache policy and an initial miss
- WHEN a successful job returns its declared outputs and cleanup is confirmed
- THEN the host publishes those guest outputs, not the host source workspace
- AND a subsequent job with the same source key restores a cache hit.

### Requirement: Observed host placement qualification

r[kiln.hosted.microvm.host_placement]

Firecracker admission MUST retain bounded host SMT, KSM, mitigation, vulnerability, kernel and resctrl facts. Untrusted-capable runners MUST reject unknown or vulnerable issue-policy kernel status. Only SMT/KSM safety refusal MAY be explicitly overridden with retained reason. The hosted shell MUST prevent untrusted and workload-bearing overlap on its physical machine, and MUST retain actual managed job overlap rather than planned pipeline peers. Opt-in cooperative guest probes MUST report raw observations and measured errors without claiming hostile isolation or fabricating separate-host qualification.

#### Scenario: Unsafe host refusal

- GIVEN a runner declared untrusted-capable and unknown kernel status
- WHEN admission occurs
- THEN no work is dispatched even with an SMT override.

#### Scenario: Cross-attempt overlap

- GIVEN an active workload-bearing attempt
- WHEN an untrusted attempt reserves the same physical host
- THEN admission refuses; overlapping same-class jobs retain symmetric peer identities.

#### Scenario: Honest measurement

- GIVEN operator-supplied guest launchers and physical placement evidence
- WHEN measurement runs
- THEN the report includes observed bit/error rates and raw samples; absent remote hosts remain an explicit missing prerequisite.

### Requirement: Reviewed cache-partition labels

r[kiln.hosted.microvm.cache_partition] A Firecracker runner MAY declare a
reviewed `cache_partition` label. Admission MUST reject a label containing both
untrusted-capable and workload-bearing runners. The receipt MUST record the
label. This is a static placement declaration and MUST NOT claim resctrl
programming, cache allocation, LLC separation or covert-channel measurement.

#### Scenario: Shared partition is rejected

- GIVEN an untrusted-capable runner and a workload-bearing runner in one label
- WHEN execution configuration is admitted
- THEN admission fails before any effect.

#### Scenario: Missing resctrl

- GIVEN a runner with a cache-partition label and no observed resctrl mount
- WHEN execution is admitted
- THEN admission fails before any effect.

#### Scenario: Label is recorded

- GIVEN a runner with a reviewed cache-partition label
- WHEN its receipt is produced
- THEN `vm.cache_partition` names the label.

### Requirement: Firecracker VMM CPU affinity

r[kiln.hosted.microvm.cpu_affinity] A Firecracker runner MAY declare a reviewed
`cpu_affinity` list. Admission MUST accept only comma-separated CPU numbers and
ranges in `0..=4095`, without duplicates, and MUST require a configured immutable
`taskset` executable when any runner declares affinity. Execution MUST start the
VMM through `taskset -c <list>` and MUST record the list in the attempt receipt.
This pins the VMM process and MUST NOT claim guest-vCPU, cache, LLC or
covert-channel isolation.

#### Scenario: Valid affinity

- GIVEN a Firecracker runner with `cpu_affinity = "0-3,8"` and a configured taskset
- WHEN the VMM starts
- THEN the command is wrapped with `taskset -c 0-3,8` and the receipt records it.

#### Scenario: Invalid affinity

- GIVEN an empty, reversed, duplicate or out-of-range CPU list
- WHEN execution configuration is admitted
- THEN admission fails before any effect.

#### Scenario: Missing taskset

- GIVEN a runner with affinity and no configured taskset
- WHEN configuration is admitted
- THEN admission fails.

### Requirement: Firecracker host preflight facts

r[kiln.hosted.microvm.host_facts] The Firecracker core MUST represent reviewed
host kernel, SMT, KSM, MPTCP and resctrl observations and classify
CVE-2026-98432 from an explicit fixed-release table plus reviewed vendor package
backports. An untrusted-capable runner MUST refuse admission when the CVE is
vulnerable or unknown, SMT is active, KSM is running, or MPTCP listeners remain
after the sysctl is disabled, unless a non-empty operator override is supplied.
The classification and refusal MUST NOT claim a patched kernel, measured
isolation, or release authority.

#### Scenario: Fixed host

- GIVEN a fixed release or a reviewed vendor backport and safe SMT/KSM/MPTCP
  facts
- WHEN untrusted admission runs
- THEN admission succeeds without an override.

#### Scenario: Unsafe host

- GIVEN a vulnerable/unknown release, active SMT, running KSM, or residual MPTCP
  listeners
- WHEN untrusted admission runs without an override
- THEN admission fails with the specific reason.

#### Scenario: Explicit override

- GIVEN an unsafe fact and a non-empty operator override
- WHEN untrusted admission runs
- THEN admission succeeds and the caller is responsible for recording the
  override.

### Requirement: Firecracker trust-domain separation

r[kiln.hosted.microvm.trust_domain] Firecracker runners MUST carry an optional
reviewed placement-domain identity. An untrusted-capable runner MUST require a
non-empty domain. A domain MUST NOT contain both untrusted-capable and
workload-bearing runners. An untrusted-capable runner MUST also carry a
non-empty reviewed host identity. A host identity MUST NOT contain both
untrusted-capable and workload-bearing runners. The selected domain and host
identity MUST be recorded in the attempt receipt. This is static runner-pool and
host-assignment separation and MUST NOT claim live cross-attempt observation,
LLC, core or cache isolation. The receipt MUST also record the other jobs in the
same attempt whose runner declares the same host identity.

#### Scenario: Cross-attempt overlap is rejected

- GIVEN an active attempt with a workload-bearing job on a declared host
- WHEN another attempt requests an untrusted job on the same declared host
- THEN admission fails before finance reservation or process effects.

#### Scenario: Attempt co-residency is recorded

- GIVEN two jobs in one attempt whose runners declare one host identity
- WHEN a job receipt is produced
- THEN it names the other job as attempt-local co-residency.

#### Scenario: Cross-attempt overlap is not claimed

- GIVEN another attempt using the same host identity
- WHEN the current attempt receipt is produced
- THEN it MUST NOT claim to have observed that other attempt.

#### Scenario: Shared domain is rejected

- GIVEN an untrusted-capable runner and a workload-policy runner in one domain
- WHEN execution configuration is admitted
- THEN admission fails before any effect.

#### Scenario: Separated domains are admitted

- GIVEN untrusted and workload-bearing runners in distinct reviewed domains
- WHEN configuration is admitted
- THEN each runner may be used and its domain is recorded in the receipt.

#### Scenario: Shared host is rejected

- GIVEN untrusted and workload-bearing runners with distinct domains but one host
- WHEN execution configuration is admitted
- THEN admission fails before any effect.

#### Scenario: Missing untrusted domain

- GIVEN an untrusted-capable Firecracker runner without a domain
- WHEN configuration is admitted
- THEN admission fails.

### Requirement: Firecracker host receipt and untrusted refusal

r[kiln.hosted.microvm.host_receipt] Every Firecracker attempt receipt MUST
include the observed host kernel/package, CVE status, SMT, KSM, MPTCP and
resctrl facts. An untrusted-capable runner MUST refuse dispatch on a vulnerable
or unknown CVE, active SMT, running KSM, or residual MPTCP listeners unless the
reviewed runner supplies a non-empty operator override. The receipt MUST retain
the observed facts regardless of override. This observation MUST NOT claim a
patched host, hostile isolation, scheduler placement, partitioning, or release
readiness.

#### Scenario: Safe host admits untrusted work

- GIVEN a fixed kernel and safe SMT/KSM/MPTCP facts
- WHEN an untrusted-capable runner dispatches
- THEN the job may start and the receipt carries the host facts.

#### Scenario: Unsafe host is refused

- GIVEN an unsafe fact and no override
- WHEN an untrusted run is admitted
- THEN dispatch fails before credentials or job effects.

#### Scenario: Override is recorded

- GIVEN an unsafe fact and a non-empty reviewed override
- WHEN the job starts
- THEN the receipt still records the unsafe observed facts and the override
  remains operator-owned authority.

### Requirement: Measured guest kernel driver surface

r[kiln.hosted.microvm.guest_surface] The guest build MUST measure the pinned
kernel image against a reviewed deny list of unused device classes using exact
Kconfig symbols and guest-closure module fragments. It MUST record each class
as `absent`, `builtin` or `loadable_module_present` together with the kernel
version, kernel-configuration BLAKE3, guest nixpkgs revision and closure module
count. An enforced class that is `builtin` or `loadable_module_present` MUST
fail the check. The checked-in inventory MUST match the measured current cohort,
so a kernel or configuration change requires a reviewed refresh. Measured-only
classes MUST be recorded without claiming they are absent.

#### Scenario: Enforced classes are absent

r[kiln.hosted.microvm.guest_surface.scenario.absent]
- GIVEN the pinned guest kernel and module closure
- WHEN the surface check runs
- THEN every enforced class MUST be `absent`
- AND the inventory MUST match the checked-in fixture

#### Scenario: A deny-listed class is present

r[kiln.hosted.microvm.guest_surface.scenario.violation]
- GIVEN a kernel that compiles an enforced class in, or a closure that ships a matching module, or a changed kernel/configuration without a refreshed inventory
- WHEN the surface check runs
- THEN it MUST fail and name the class
- AND no build or runtime result MAY be reinterpreted as a pass

#### Scenario: Live guest sysfs agrees

r[kiln.hosted.microvm.guest_surface.scenario.escape]
- GIVEN a booted guest
- WHEN the escape suite probes the enforced classes' sysfs entries
- THEN the entries MUST be absent
- AND the probe MUST fail closed if any appears
