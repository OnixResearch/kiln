# Verification evidence

## Result

Firecracker runners can declare a reviewed CPU list. Admission validates it and
requires an immutable `taskset`; execution wraps the VMM with `taskset -c` and
records `vm.cpu_affinity`.

## Evidence

- Hosted library tests: 65 passed.
- Workspace `--all-targets`: pass.
- Hosted Nickel typecheck: pass.
- `cairn validate`: zero issues.

## Non-claims

`taskset` pins the VMM process, not guest vCPU placement. Cache/LLC partitioning
and covert-channel measurement remain open.
