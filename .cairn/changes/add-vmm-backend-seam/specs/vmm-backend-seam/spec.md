# VMM Backend Seam Specification

## ADDED Requirements

### Requirement: Pure backend capability seam

r[kiln.microvm.backend.seam] `kiln-microvm` MUST expose a pure `VmmBackend`
trait returning backend kind, capabilities, boot arguments, vsock listener path
and identity inputs. Every admissible backend MUST declare
`NetworkCapability::NoDevice`; a backend that cannot declare it MUST be refused.
The seam MUST NOT perform I/O or grant an isolation claim. Admission MUST read
the backend capability declaration for arch, snapshot and guest-network
decisions instead of matching a closed kind. A runner configuration MAY carry
an explicit `network` request, but only `none` is admissible; any other value
MUST be refused before an effect.

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
