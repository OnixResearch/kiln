# Implementation Plan Review Specification

## Purpose

Defines the `implementation-plan-review` capability.

## Requirements

### Requirement: Opt-in plan review before implementation admission

r[kiln.implementation.plan_review] Kiln MUST keep implementation admission unchanged unless an operator explicitly configures a plan-review policy that names the selected repositories and reviewers. A plan MUST be pinned by a BLAKE3 content identity over its exact steps and author and bound to one exact current work identity. Each revision MUST name the plan it replaces. A policy reviewer MAY annotate a named region of a pinned plan, either a whole step or a byte span of its text, with a comment, a step deletion or a suggested replacement, and MAY mark the annotation blocking. A reviewer MUST NOT annotate, resolve or accept their own plan. Accepting a plan MUST require the latest revision and no unresolved blocking annotation on any revision, and MUST NOT claim, reserve or dispatch an implementation attempt. While a selecting policy is configured, work whose exact version has no accepted latest plan MUST be blocked from claims with a plan-review reason. Kiln MUST project the annotations, their regions, quoted text and resolutions as structured feedback for the plan's author, and MUST compare a revision with the plan it replaces.

#### Scenario: Default admission is unchanged

r[kiln.implementation.plan_review.scenario.default]
- GIVEN no configured plan-review policy, or a policy that does not select the work's repository
- WHEN work is admitted, ranked and claimed
- THEN its eligibility, ranking and claim MUST be unchanged, and the stored work ledger MUST not gain plan-review fields

#### Scenario: An unaccepted plan blocks the claim

r[kiln.implementation.plan_review.scenario.blocked]
- GIVEN a policy that selects the work's repository
- WHEN the exact work version has no plan, an unreviewed or changes-requested latest plan, or an unresolved blocking annotation
- THEN the work MUST be blocked with a plan-review reason and MUST NOT be claimed, and accepting the plan alone MUST NOT create an attempt

#### Scenario: Revisions are compared and feedback is returned

r[kiln.implementation.plan_review.scenario.revision]
- GIVEN annotations on a pinned plan
- WHEN its author proposes a revision
- THEN the revision MUST name the previous pinned plan, the comparison MUST list kept, changed, added and removed steps, and the feedback MUST list every annotation with its region, quoted text, kind and resolution

#### Scenario: Malformed or self-serving review is refused

r[kiln.implementation.plan_review.scenario.refused]
- GIVEN a plan for another or superseded work version, a revision that skips or forks the thread, an annotation on a missing region or span, a deletion of part of a step, a reviewer outside the policy or the plan's own author, or an acceptance with unresolved blocking annotations
- WHEN the command is applied
- THEN it MUST be refused without changing the review thread

### Requirement: Frozen plan-review comparison

r[kiln.implementation.plan_review.comparison] Kiln MUST keep a frozen, typed set of plan-review cases and a deterministic receipt that replays each case through the real plan-review commands. For each case the receipt MUST report whether review changed the accepted plan, prevented an implementation that accepting the first plan unchanged would have admitted, or left it unchanged. A case without plan bytes or without a review decision MUST be reported as unavailable with its reason, never as a pass. The receipt MUST state each case's provenance and MUST NOT claim a measured quality gain.

#### Scenario: Frozen cases are classified

r[kiln.implementation.plan_review.comparison.scenario.frozen]
- GIVEN the frozen cases, including a missing agent capture and a plan that never reached review
- WHEN the receipt is produced
- THEN each case MUST be changed, prevented, unchanged or unavailable with a reason, the counts MUST match the cases, and the receipt MUST equal the committed expectation

### Requirement: Structured Plan Review annotation and receipt surface in the console

r[kiln.implementation.plan_review_console_surface]
The console MUST render structured implementation plans (`docs/plan-review.md`) inside the Slide-Over Inspector and `2.1 Delivery & Cairn` with step-level annotations, accept/request-changes feedback controls, and recorded plan-review receipt digests bound to the exact plan revision. Accepting or rejecting a plan MUST fail closed when the underlying plan revision has changed since observation.

#### Scenario: Stale plan revision blocks acceptance

r[kiln.implementation.plan_review_console_surface.scenario.stale_plan]
- GIVEN an operator reviewing a `plan_not_accepted` item whose plan revision changed concurrently
- WHEN the operator submits plan acceptance for the prior revision
- THEN the service MUST reject the stale acceptance with an explicit revision-mismatch diagnostic and require inspecting the new plan revision
