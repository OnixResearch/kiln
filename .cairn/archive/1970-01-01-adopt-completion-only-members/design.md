## Context

`observation::refresh` (`crates/kiln-web-service/src/implementation_campaign/observation.rs`) turns one native Cairn snapshot per selected change into member observations. For a completion-only snapshot no canonical work is admitted, and `stage_for_tasks` returns `Blocked`. Required native units are `<change>/task:<id>` or `<change>/change:<name>`, the same identities as the `cairn_task` aliases of canonical work. The verification owner (`member_from_owner`) and the delivery owner (`delivery::readback`) already map a verified or integrated attempt into member state: work, attempt and fence binding, source and subject identities, candidate and verifier freshness, required-check satisfaction and evidence identities. `integrated_successor_permitted` already matches route integration receipts to an exact attempt.

## Goals and Success Contract

A freshly selected completion-only member whose implementation went through Kiln reaches its integrated stage from canonical evidence alone and then follows the existing lifecycle path. Anything less stays an explicit, specific blocker.

## Decisions

### Decision: a pure decision over extracted unit facts

**Choice:** a thin extraction reads, for each required unit, the current canonical work naming it and the latest attempt of that work's canonical key, plus the route's integration fact for that attempt (the GitHub integration receipt or the native Git receipt bound to the same work identity, revision, attempt and fence). A pure `decide` function then requires, unit by unit: current work, an attempt bound to that work's identity, the `Verified` phase with a retained successful verification run holding exactly one passing `cairn-authoritative-completion` check of kind `cairn-agent-completion-v2`, an integration fact, containment of the integrated result in the current target, and an unchanged change source since that result. It binds the unit whose integrated result is, or descends from, every other integrated result.

**Rationale:** the decision is the part where a mistake adopts unearned evidence, so it is pure and tested case by case. Extraction reuses the exact receipt bindings already used for integrated successors.

**Alternative:** adopting the latest verified attempt without integration. Completion-only means the native tasks are done in the reviewed source; only an integrated result places that source in the target, so integration is required.

### Decision: containment and change-source applicability from the target repository

**Choice:** the shell opens the route's target repository and requires the integrated result commit to equal or be an ancestor of the current target commit, and the `.cairn/changes/<change>` tree entry to be identical at both commits. Missing objects or unreadable commits become blockers that quote the failure.

**Rationale:** the native snapshot digest moves whenever tasks are checked, so it cannot show applicability. The target history can: an integrated result that the target contains, with the change source untouched since, is the source the native snapshot reports as complete.

### Decision: apply the binding after the per-work reset

**Choice:** the completion-only branch sets the adopted work identity, key and source identity with the integrated stage. The attempt binding, freshness, required-check satisfaction, evidence and removal of adopted units from the remaining obligations are applied after `refresh` clears attempt facts for a changed work, and a re-observation of the same work keeps any later stage through the existing progress preservation.

**Rationale:** the reset exists for work that changed; applying the binding after it keeps one code path for both fresh adoption and re-observation.

## Risks / Trade-offs

- A member observed earlier without adoptable evidence that later gains it changes its source identity, which the campaign owner already turns into an explicit impact and replan.
- Changes implemented outside Kiln, or only partly through it, stay blocked; that is the specified non-claim, not a regression.
- Lifecycle source preparation is still expected in the reviewed candidate; adoption does not prepare it.
