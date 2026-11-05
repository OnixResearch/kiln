# Verification evidence

## Result

Added the pure `VmmBackend` seam, Firecracker and Cloud Hypervisor capability
declarations, shared vsock listener path and the Cloud Hypervisor phase-1
cold-boot argv.

## Evidence

- `cargo test -p kiln-microvm backend`: 3 passed.
- Hosted library tests: 67 passed, including capability-driven admission and
  explicit network refusal.
- Workspace `--all-targets`: pass.
- `cairn validate`: zero issues.

## Non-claims

Cloud Hypervisor execution is not wired. Admission is not yet driven by
capabilities. No Cloud Hypervisor escape-suite, confinement or latency evidence
exists.
