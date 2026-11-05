# Verification evidence

## Result

Added the pure `VmmBackend` seam, Firecracker and Cloud Hypervisor capability
declarations, shared vsock listener path and the Cloud Hypervisor phase-1
cold-boot argv.

## Evidence

- `cargo test -p kiln-microvm backend`: 3 passed.
- Hosted library tests: 68 passed, including capability-driven admission,
  explicit network refusal and snapshot refusal.
- MicroVM library tests: 36 passed, including snapshot identity v2 and Cloud
  Hypervisor pause/snapshot/restore plans.
- Workspace `--all-targets`: pass.
- `cairn validate`: zero issues.

## Non-claims

Cloud Hypervisor execution is not wired. Admission is not yet driven by
capabilities. No Cloud Hypervisor escape-suite, confinement or latency evidence
exists.
