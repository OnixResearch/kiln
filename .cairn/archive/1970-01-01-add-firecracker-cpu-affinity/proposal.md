## Why

Issue #28 still lacks core partitioning. An operator could not pin a Firecracker
VMM to a reviewed CPU set, so co-resident trust domains shared every core.

## What Changes

- Add optional `cpu_affinity` to Firecracker runners and optional `taskset` to
  the execution configuration.
- Validate the CPU-list syntax and reject duplicate or out-of-range CPUs.
- Start the VMM through the configured `taskset` and record the list in
  `vm.cpu_affinity`.
- Keep guest vCPU placement and cache partitioning out of scope.

## Impact

- **Files**: `crates/kiln-hosted/src/execution.rs`,
  `crates/kiln-hosted/src/microvm.rs`, hosted Nickel profiles,
  `docs/hosted-ci.md`.
- **Testing**: affinity parser tests, hosted tests, Nickel checks.
- **Authority**: pins the VMM process; does not claim guest-vCPU, cache or LLC
  isolation.
