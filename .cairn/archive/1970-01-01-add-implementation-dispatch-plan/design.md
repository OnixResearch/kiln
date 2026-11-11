## Decisions

The plan is a separate pure type rather than fields on `ImplementationWork`, so
one work unit can have distinct attempts with different budgets/profiles while
each remains bounded by the work's admitted paths, effects and minutes.

Admission compares plan base to both the work base and the frozen attempt base.
Because the attempt freezes the base at claim, a later work edit cannot widen an
active dispatch. Paths and effects are subsets, never merged.

The plan identity is computed after all checks over a canonical encoding. Only
the identity is passed to `reserve_dispatch`, so the reservation remains
independent of Lattice types while still binding every admitted fact.

## Non-claims

Plan admission does not prove provider capability, execute a Lattice workflow,
create a workspace or authorize publication. Network and credential fields are
refused for this route rather than interpreted as grants.
