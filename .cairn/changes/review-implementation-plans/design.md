## Context

Implementation admission today has these parts:

- `WorkLedger` admits exact work versions and computes `Eligibility`. `BlockReason` covers missing acceptance obligations, missing dependencies and cycles.
- `WorkLedger::ranked` returns only `Ready` work, and `Implementer::claim_next` claims from it. The fair-admission path (`kiln-implementer::admission`) marks any work that is not `Ready` as `Denied` before it probes `AttemptLedger::claim`.
- `Implementer` stores the work and attempt ledgers as versioned JSON rows and applies every mutation in one immediate SQLite transaction. `kiln-implementer` exposes operator subcommands that read a JSON command file and print a JSON result.

Plannotator, read at `d478ccf9c0b2b9a0ede1f68dc15c5485e80fb71d`, treats a plan as an annotatable artifact: reviewers comment, delete or replace regions, the agent receives structured feedback, and a revised plan is shown against the previous one. Kiln takes that review shape, not the tool.

## Goals and Success Contract

An operator can opt selected repositories into plan review. For that work, an agent's plan is pinned, reviewed, revised and accepted before any claim. Feedback and revision comparisons are retained. Without the policy nothing changes, including the stored ledger. A frozen receipt records, case by case, whether review changed or prevented what accepting the first plan would have admitted.

## Decisions

### Decision: the gate is a work-eligibility reason

**Choice:** while a configured `PlanReviewPolicy` names the work's repository, `WorkLedger` eligibility adds `BlockReason::PlanNotAccepted` unless the plan gate for the exact work identity is `Accepted`. `AttemptLedger::claim` is unchanged.

**Rationale:** every claim path already requires `Ready` work: `ranked`, `claim_next`, and fair-admission inputs, which become `Denied`. One rule therefore covers all of them, and no claim signature or caller changes. Accepting a plan only records a decision; a claim still goes through the existing admission path.

### Decision: review state lives in the work ledger, absent by default

**Choice:** `WorkLedger` gains `plan_review: Option<PlanReviewPolicy>` and `plan_threads` keyed by exact work identity. Both use `serde(default)` and are skipped when empty. Every change goes through `WorkLedger::apply_plan_review(PlanReviewCommand)`, which `Implementer::plan_review` runs in the existing transaction.

**Rationale:** the gate and its evidence commit atomically with the work they gate. Stored ledgers without plan review read and write unchanged. A new work version has a new identity, so it needs its own accepted plan.

### Decision: pinned plans and a linear revision thread

**Choice:** an `ImplementationPlan` binds the work identity, a 1-based revision, the identity of the plan it replaces (absent only for revision 1), the author and 1 to 64 steps with unique ids. Its identity is BLAKE3 over every field. A proposal must be the next revision of the thread for a current work version. Re-proposing an identical pinned plan is idempotent, and any other duplicate, skip or fork is refused.

### Decision: annotations, resolutions and decisions

**Choice:**

- A `PlanRegion` is a step id with an optional byte span `[start, end)` on character boundaries.
- An annotation is a comment, a step deletion (whole step only) or a suggested replacement, and may be blocking.
- Annotating, resolving and deciding require a reviewer named by the policy who is not the plan's author.
- A resolution names one unresolved blocking annotation and the latest revision it was resolved against.
- A decision applies to the latest revision. An acceptance is refused while any blocking annotation in the thread is unresolved.

The gate is evaluated in this order:

1. no plan;
2. unresolved blocking annotations;
3. the last decision on the latest revision: none, changes requested or accepted.

Bounds: 16 revisions, 256 annotations, 64 decisions, 4,096-byte texts, 32 reviewers and 64 repositories.

### Decision: feedback and comparison are projections

**Choice:** `PlanFeedback` lists every annotation in thread order with its revision, region, quoted text from the annotated plan, kind, blocking flag and resolving revision, plus the gate. `compare_plans` reports kept, changed, added and removed step ids and whether kept steps were reordered. Neither changes state.

### Decision: a frozen, replayed comparison

**Choice:** `config/plan-review-contracts.ncl` types the cases. Each case gives its provenance (`synthetic` or `agent_capture`), its plan revisions, and annotations, resolutions and decisions by revision. `config/fixtures/plan-review-positive.ncl` exports `fixtures/plan-review/cases.json`, and the check script refuses a stale export. `plan_review_receipt(work, cases)` replays each case through `apply_plan_review` on a fresh ledger with a policy for the work's repository:

| Replay result | Outcome |
| --- | --- |
| No plan | `unavailable`, `missing_plan` |
| No decision | `unavailable`, `not_reviewed` |
| Gate not accepted | `prevented`: accepting revision 1 would have admitted it |
| Accepted plan differs from revision 1 | `changed`, with the comparison |
| Accepted revision 1 | `unchanged` |

A replay refusal fails the receipt. The receipt carries per-outcome counts and the identity of the cases. A core test compares it with `fixtures/plan-review/expected.json`.

## Risks / Trade-offs

- The cases are synthetic. They show the mechanism and the accounting, not a quality gain, and a captured agent case stays unavailable until real plan bytes are added.
- Plan review adds no UI. Reviewers use the operator CLI, and returning feedback to an agent is the calling shell's job.
- Under a selecting policy, a work version without an accepted plan is blocked even when no planning job exists. That is the opt-in contract.

## Verification Strategy

- Core unit tests cover:
  - plan identity, binding and revision rules;
  - every region and annotation rule, including self-review and reviewers outside the policy;
  - resolutions, decisions and gate states;
  - feedback and comparison;
  - unchanged default eligibility and serialized form;
  - the blocked, then accepted, eligibility under a policy.
- The frozen receipt test compares with the committed expectation.
- An `Implementer` test proves `claim_next` returns nothing until the plan is accepted, and that acceptance itself creates no attempt.
- A CLI test drives `plan-review` and `plan-feedback`.
- The Nickel gate checks the positive export, its freshness and a refused negative fixture.
- Clippy and scoped formatting run, and the Cairn gates and validation run.
