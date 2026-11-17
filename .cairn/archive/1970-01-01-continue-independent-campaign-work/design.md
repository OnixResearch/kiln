## Context

`CampaignState::next_action` in `kiln-core` selects one decision. After the campaign barriers it sorts candidate members, blocked members last and then by member key. It returns the first decision derived for any member: a required-review wait, a missing-context blocker, a replan, a delivery or endpoint-evidence wait, or an owner action. Dependency-blocked members were already filtered out, so the existing independence test passed. A member-local wait, however, ends selection for every later member.

The native controller in `kiln-web-service` runs a bounded loop. Each iteration refreshes owner observations, recovers or claims for the selected decision, then calls `drive_one`. The loop stops when the selected decision has no effect. Its recovery step checks an ambiguous dispatch only when the selected decision is the executing member's own wait. Claims go through canonical admission (`claim_campaign_work`), which refuses a second active or unreconciled repository writer and owns budget and lineage checks.

## Decisions

### Decision: evaluate members, then select

**Choice:** `CampaignState::evaluate` returns `CampaignEvaluation { decision, basis, holds }`. A private `MemberEvaluation` classifies each unsatisfied member in the existing order:

- `Action`: a dispatchable owner effect (`CampaignOperation::is_owner_effect`).
- `Hold`: a condition in the member's own scope, such as a review, missing required context, stale input, replan, milestone successor, delivery capability, endpoint evidence, unknown-outcome reconciliation or an active attempt.
- `Deferred`: a wait on an unsatisfied dependency, on the shared writer slot, or on incomplete or absent observation, or a stage that admits no campaign action.

Selection is the campaign barrier if one applies, else the first `Action`, else the first `Hold`, else the existing no-eligible-member decision. `holds` keeps every `Hold` and `Deferred` decision in selection order. `next_action` returns `evaluate().decision`.

**Rationale:** each condition keeps its member, reason, owner and effect intent, so another member's action cannot erase or satisfy it. When no member can act, the surfaced decision is the one the previous first-return loop produced: deferred waits were previously skipped, and actions and holds keep their order. A deferred wait is never surfaced in place of the root hold it waits on. Its root is either another member's hold or the active writer, which the driver's recovery check reads.

### Decision: writer claims consult the writer slot

**Choice:** `AuthorArtifact`, `RepairArtifact`, `Code`, `RepairCode` and `RetryInfrastructure` (`requires_writer_claim`) are deferred while another member is claimed, executing, a candidate or verifying. `Verify`, `Publish`, `Integrate` and `Lifecycle` act on a member's own attempt or verified candidate and stay admissible.

**Rationale:** previously an active member's wait was always sorted ahead of blocked members and returned first. That ordering kept repair and retry from starting a second writer. Once holds no longer end selection, the guard must be explicit. It mirrors canonical admission's same-repository writer refusal, so the controller does not select a claim that admission is certain to refuse. Canonical admission remains the final owner and still refuses unreconciled writers and exhausted budgets.

### Decision: one bounded loop, native steps behind a shell

**Choice:** `kiln_implementer::campaign::drive_bounded` owns the loop and stop rule. The native `NativeCampaignConsumer` implements `CampaignDriveShell` with `refresh`, `recover_executing` and `claim`, using the previous `bind_selected_claim` code. The loop claims only for a selected writer operation. When the basis is `MemberHold`, it asks the shell to recover each executing member among the holds.

**Rationale:** the same loop is exercised against the real SQLite store with scripted owners. Those tests count claims, dispatches, reconciliations and recoveries across a restart. Recovering any executing member when no member can act keeps another member's hold from deferring an ambiguous dispatch indefinitely. When some member can act, independent work proceeds and the executing member still holds the writer slot.

### Decision: show holds where operators already read the campaign

**Choice:** the v1 selected-campaign projection adds `selection_basis` and per-member `selection_hold`, and the drive output adds `basis` and `holds`. The console renders each member's hold. Older projections without the field render "none recorded".

**Rationale:** these are additive fields of a read-only projection with no compatibility consumer that rejects unknown keys. A member's own row now explains why it is not selected.

## Risks / Trade-offs

- A member-level unknown-outcome reconcile is a hold. When surfaced, `drive_one` still errors if no campaign effect is retained, as before. Fixing that recovery path is separate work.
- An executing member with an ambiguous dispatch is recovered only after no member can act. Independent publication and verification can proceed first. Writer admission stays closed because the active attempt holds the slot.
- Scripted owners prove the loop's ordering and persistence, not native worker, model or provider behavior.
