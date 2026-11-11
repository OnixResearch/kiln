## Decisions

The base is optional because GitHub webhook intake cannot resolve an exact target
without a source read; forcing a value would invent one. Producers that resolve a
base set it, and the canonical identity changes when it does.

Claims copy the base into the attempt. Supersession never rewrites attempt fields,
so the frozen base remains auditable even after an AbortActive or FinishActive
transition.

## Non-claims

This does not resolve a branch to a revision, verify that the base exists, or
move the target. A missing base is explicit, not evidence of the default branch.
