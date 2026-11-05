## Context

The hosted execution enum and shell are Firecracker-shaped. A trait lets the
pure core declare capabilities and launch plans without performing I/O.

## Decisions

### Trait surface

`VmmBackend` returns `kind`, `Capabilities`, boot arguments, the shared
vsock listener path and identity inputs. `Capabilities` includes only
`NetworkCapability::NoDevice`, snapshot support, reviewed arches, confinement
kinds and per-job scope.

### Cloud Hypervisor phase 1

The Cloud Hypervisor implementation is a pure plan surface. Its argv omits
`--net` and `--api-socket`, keeps the source disk read-only and uses the same
vsock listener path. Execution is not wired yet; Firecracker remains the only
per-job shell.

## Risks

- The trait is not yet used for admission, snapshot identity v2 or receipts
  beyond the backend kind.
- Cloud Hypervisor argv and guest PCI transport remain unverified on a host.
