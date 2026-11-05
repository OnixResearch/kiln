# Verification evidence

## Result

Added the pure `VmmBackend` seam, Firecracker and Cloud Hypervisor capability
declarations, shared vsock listener path and the Cloud Hypervisor phase-1
cold-boot argv.

## Evidence

- `cargo test -p kiln-microvm backend`: 3 passed.
- Hosted library tests: 69 passed, including capability-driven admission,
  explicit network refusal, snapshot refusal and Cloud Hypervisor runner
  construction.
- MicroVM library tests: 37 passed, including snapshot identity v2, Cloud
  Hypervisor pause/snapshot/restore plans and backend process-name truncation.
- Hosted documentation now states that Cloud Hypervisor phase 1 is plan-only
  and lists its claims and non-claims.
- Both guest images build locally: `microvm-guest` and `microvm-guest-ch`.
  Their manifests are `kiln.microvm.guest-image.v2` with a `vmm` entry.
- Workspace `--all-targets`: pass.
- `cairn validate`: zero issues.

## Non-claims

Cloud Hypervisor execution is not wired. Admission is not yet driven by
capabilities. No Cloud Hypervisor escape-suite, confinement or latency evidence
exists.
