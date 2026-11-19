# Lattice Worker Guest Specification

## Purpose

Defines the `lattice-worker-guest` capability.

## Requirements

### Requirement: Pinned Lattice worker guest
r[kiln.lattice.worker.guest]
Kiln MUST build a Lattice worker guest from Nix with a reviewed kernel, an immutable initramfs and a pinned Cloud Hypervisor. The initramfs MUST contain the pinned Lattice executable, the admitted workflow definitions and the guest shell. A `kiln.lattice-worker-guest.v1` manifest MUST bind the architecture, kernel, initramfs and VMM BLAKE3 identities, the actual VMM version output, the guest environment identity, the device profile and the resource limits. The guest environment identity is the BLAKE3 identity of a `kiln.lattice-worker-environment.v1` record. That record binds the Lattice revision and binary, the guest shell and busybox, and, for each workflow, its definition bytes and the effect requirements that the pinned Lattice binary reports. Mutable workspace, Lattice state, guest identity and effect authority MUST be per attempt and MUST NOT be in the image.

#### Scenario: Exact identities recorded
r[kiln.lattice.worker.guest.scenario.identity]
- GIVEN the `lattice-worker-guest` package build
- WHEN its manifest is loaded by the hosted shell
- THEN every recorded object MUST re-hash to its manifest identity, and the embedded environment MUST hash to the recorded environment identity, or the manifest MUST be refused

#### Scenario: Crossed identities refused before dispatch
r[kiln.lattice.worker.guest.scenario.refused]
- GIVEN an attempt request naming a different image, environment, workflow, workflow revision or workspace identity, or a spent deadline
- WHEN the host admits it, or when the guest admits the received dispatch against its baked environment
- THEN admission MUST refuse it before any VM, workspace or Lattice effect, and no workflow MUST run

#### Scenario: Independent attempts
r[kiln.lattice.worker.guest.scenario.isolated]
- GIVEN two admitted attempts on the same immutable guest artifacts
- WHEN both run
- THEN they MUST use distinct workspace images, Lattice stores, guest identities and authority profiles, and a workspace already used by one attempt MUST be refused for the other

#### Scenario: Production executor in the guest
r[kiln.lattice.worker.guest.scenario.executed]
- GIVEN an admitted attempt for a multi-step workflow on a KVM host
- WHEN the guest receives its single dispatch
- THEN the pinned Lattice headless executor MUST run the workflow under the per-attempt authority profile, persist its run in the workspace, and report the observed run summary

### Requirement: Single-host retained session qualification
r[kiln.lattice.worker.session]
The hosted shell MUST run the Lattice worker guest through the existing retained Cloud Hypervisor shell, without a second VMM supervisor. Attempt dispatch MUST be journalled before it is sent, MUST happen at most once per attempt, and MUST require an observed `Running` VMM. Single-host qualification MUST report missing KVM, VMM, guest or host-tool prerequisites explicitly, and MUST fail rather than skip when required.

#### Scenario: Real lifecycle
r[kiln.lattice.worker.session.scenario.lifecycle]
- GIVEN the pinned CH 54.0 VMM and a raw per-attempt workspace on a KVM host
- WHEN the session cold boots, resumes, dispatches, pauses, snapshots, retires, reopens its checkpoint and restores
- THEN each state MUST be adopted only from native VMM readback with observed confinement, capacity MUST be released only after observed cleanup, and the restored guest MUST continue the same attempt

#### Scenario: Dispatch at most once
r[kiln.lattice.worker.session.scenario.dispatch-once]
- GIVEN an attempt whose dispatch intent is already journalled
- WHEN the controller restarts or the dispatch is repeated
- THEN the shell MUST return the recorded observation, or reconciliation if none was recorded, without sending another dispatch

#### Scenario: Stale or retired owner
r[kiln.lattice.worker.session.scenario.stale]
- GIVEN a retained session whose recorded owner, placement or state differs from a command
- WHEN a stale pause or resume or a cold start after retirement is requested
- THEN the shell MUST refuse it before any host effect

#### Scenario: Missing prerequisites
r[kiln.lattice.worker.session.scenario.unavailable]
- GIVEN a host without KVM, the guest package, the systemd user manager or the workspace inspection tool
- WHEN the qualification test runs
- THEN it MUST print an explicit skip reason, or fail when `KILN_MICROVM_REQUIRED=1`; it MUST NOT print a qualification receipt
