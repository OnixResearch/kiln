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
- MicroVM library tests: 38 passed, including snapshot identity v2, Cloud
  Hypervisor pause/snapshot/restore plans, process-name truncation and the
  unfiltered-control-thread policy.
- Hosted documentation now records the passing Cloud Hypervisor cold-boot and
  escape-suite host result.
- Both guest images build locally: `microvm-guest` and `microvm-guest-ch`.
  Their manifests are `kiln.microvm.guest-image.v2` with a `vmm` entry.
- Cloud Hypervisor host test: canonical outcome matched namespaces, and the
  escape suite denied network, namespace and vsock egress.
- Workspace `--all-targets`: pass.
- `cairn validate`: zero issues.

## Non-claims

Cloud Hypervisor execution is not wired. Admission is not yet driven by
capabilities. No Cloud Hypervisor escape-suite, confinement or latency evidence
exists.
