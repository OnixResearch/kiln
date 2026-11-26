## Context

`integrate-retained-mobility` already launches guests, records pause and resume, takes paused checkpoints, transfers them over an authenticated Iroh stream, and builds a paused handoff. Its configured Aspen provider refuses every cross-host effect because current-placement compare-and-swap and resource fencing do not exist.

## Decisions

### Decision: Stopped-guest migration, not live migration

**Choice:** Pause, checkpoint, transfer, then restore on the destination. Live migration is out of scope.

**Rationale:** Every step has a durable record and an observable result. Live migration needs a qualified streaming provider and gives no better ownership guarantee.

### Decision: Aspen is the only placement authority

**Choice:** The move is one Aspen placement transition, from the source owner to the destination owner, guarded by the current placement version. Kiln restores on the destination only after Aspen grants it. It retires the source only after Aspen fences the source's resources.

**Rationale:** Without an external owner, a lost reply can leave two guests writing the same worktree. A fence is what makes the duplicate inert.

### Decision: Ordered, recorded steps

**Choice:** Each step is `requested`, then `observed` or `uncertain`, recorded before its effect: pause, checkpoint, transfer, placement grant, restore, reconnect, source fence, retirement. An uncertain step blocks later steps until it is reconciled. Reconciliation reads native state and never repeats an effect.

**Rationale:** This reuses the existing retained-operation journal and keeps occupied capacity counted until cleanup is observed.

## Risks / Trade-offs

- Restore requires compatible CPU features, the same Cloud Hypervisor version, and the guest disk on the destination. Destination admission checks the recorded compatibility profile before any pause.
- The agent's in-flight model and tool calls are lost and reported as uncertain. The pause is visible to the operator.
- If the destination restore fails after the placement grant, the guest stays paused and Aspen-owned; the operator chooses roll back or abandon. Kiln does not silently restart the source.
