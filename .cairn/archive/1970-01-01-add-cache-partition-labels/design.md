## Context

Trust domains and host identities already separate untrusted and workload-bearing
runners statically. Cache placement had no reviewed identity, so an operator
could not declare which runners are intended to share cache resources.

## Decisions

`cache_partition` is a reviewed label. A label may not contain both
untrusted-capable and workload-bearing runners. The receipt records it next to
`host_identity` and `trust_domain`.

This is intentionally not enforcement: Kiln does not program resctrl, create
CAT/MBA allocations, pin guest vCPUs or measure covert channels. A truthful host
label still depends on the operator.

## Risks

- Two labels may map to one physical cache domain.
- No cache-allocation or side-channel measurement exists.
