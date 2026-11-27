## Context

Kiln owns CI records, claims and attempts. Cairn, review providers, deployment targets, and Lattice retain their independent observations and effect authority.

## Decisions

### Decision: Project owner records separately

**Choice:** Read bounded owner records, join only by exact identity, and use existing admitted actions. Distinguish an absent or unavailable source from successful empty reads.

**Rationale:** A merge is not a deployment, an uncertain effect is not a failed effect, and worker activity is not progress.

## Risks / Trade-offs

- No unverified link, implicit retry, or provider action from a read-side projection.
