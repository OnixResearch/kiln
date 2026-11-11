# Vmm Backend Seam Specification

## Purpose

Defines the `vmm-backend-seam` capability.

## Requirements

### Requirement: Pure backend capability seam

r[kiln.microvm.backend.seam] `kiln-microvm` MUST expose a pure `VmmBackend`
trait returning backend kind, capabilities, boot arguments, vsock listener path
and identity inputs. Every admissible backend MUST declare
`NetworkCapability::NoDevice`; a backend that cannot declare it MUST be refused.
The seam MUST NOT perform I/O or grant an isolation claim. Admission MUST read
the backend capability declaration for arch, snapshot and guest-network
decisions instead of matching a closed kind. A runner configuration MAY carry
an explicit `network` request, but only `none` is admissible; any other value
MUST be refused before an effect. A warm pool MUST be refused when the backend
declares `snapshot = false`, and an unreviewed host architecture MUST be refused
when it is not in `Capabilities::arch`. The receipt MUST record the VMM kind,
reviewed version and binary BLAKE3. Warm snapshot identity MUST be
`kiln.microvm.snapshot.v2` and MUST change when the VMM kind, version or binary
changes, so a snapshot is never restored under another VMM identity. The Cloud
Hypervisor phase-1 plan MUST use one `vm.pause` followed by one `vm.snapshot`
with a `destination_url`, and restore with `--restore
source_url=file://...,resume=false`; it MUST NOT pass `--net`. Documentation MUST state that phase 1 is not executable
and MUST list the absent escape-suite, confinement, latency, CET and
device-model evidence. The guest image manifest MUST be
`kiln.microvm.guest-image.v2` with a `vmm` entry containing kind, version and
binary identity; the Cloud Hypervisor guest MUST load `virtio_pci` instead of
`virtio_mmio`. A `kind = "cloud-hypervisor"` runner MUST be admitted only on
its declared arch, with no network request, no cache policy and no warm pool;
its cold-boot launch MUST use the pure Cloud Hypervisor argv and the same guest
protocol. Warm restore MUST remain refused until it is wired and observed. The
`ConfinementObserved` gate MUST compare the expected backend process name,
using Linux's 15-byte `comm` truncation, rather than a Firecracker literal.
The gate MUST aggregate every VMM thread. A Cloud Hypervisor control thread MAY
be unfiltered only when at least one worker thread has a seccomp filter; the
receipt MUST record the thread count and filtered-thread count. A host test MUST
run the same pipeline and escape suite on Cloud Hypervisor, including warm-pool
restore, and compare the canonical outcome with the namespace backend.

#### Scenario: No guest network

- GIVEN a backend capability declaration
- WHEN it is admitted
- THEN its network capability MUST be `no_device`.

#### Scenario: Shared vsock listener

- GIVEN a Firecracker or Cloud Hypervisor vsock socket
- WHEN the listener path is computed
- THEN it MUST be `<socket>_10240`.

#### Scenario: Cloud Hypervisor cold boot has no network or API socket

- GIVEN the Cloud Hypervisor phase-1 cold-boot plan
- WHEN its argv is inspected
- THEN it MUST contain `--seccomp true`, MUST NOT contain `--net` and MUST NOT
  contain `--api-socket`.
