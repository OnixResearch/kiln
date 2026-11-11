## Context

`systemd-run` starts one attempt unit and then executes the Firecracker command.
Per-runner affinity therefore has to be applied to the VMM process, not the unit.

## Decisions

### taskset wrapper

When a runner declares `cpu_affinity`, `spawn_vmm` prefixes the command with the
configured immutable `taskset` and `-c <list>`. Configuration admission requires
`taskset` whenever any Firecracker runner declares affinity. The list is bounded
to CPUs `0..=4095`, with comma-separated ranges and no duplicates.

### Receipt

The `vm` object records the affinity list. This proves the requested placement,
not the guest's vCPU placement or cache state.

## Risks

- `taskset` confines the VMM thread/process affinity mask; Firecracker may create
  threads that inherit it, but guest vCPU-to-host mapping and cache partitions
  are not controlled.
- A host with fewer CPUs than the list admits fails at execution time.
