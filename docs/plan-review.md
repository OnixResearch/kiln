# Plan review before implementation admission

Plan review is off by default. When an operator configures it for selected
repositories, an agent's plan for a work version is reviewed and accepted
before Kiln claims an implementation attempt for that version. The review shape
follows [Plannotator](https://github.com/backnotprop/plannotator): a plan is an
annotatable artifact, reviewers comment on, delete or replace parts of it, the
author gets structured feedback, and a revision is compared with the plan it
replaces. Kiln adopts the shape, not the tool, and adds no review UI.

## Policy

`PlanReviewPolicy` names the repositories whose work needs an accepted plan and
the reviewers who may act. The operator sets or clears it with a `configure`
command. Without a policy, or for a repository the policy does not name:

- eligibility, ranking and claims are unchanged;
- the stored work ledger gains no plan-review fields;
- every other plan-review command is refused (`implementation_plan_review_disabled`
  or `implementation_plan_work`).

## Plans, annotations and decisions

| Item | Rules |
| --- | --- |
| Plan | Bound to one exact current work identity, with 1 to 64 uniquely named steps. Its identity is BLAKE3 over every field. Revision 1 names no previous plan; every later revision names the plan it replaces. Proposing an identical plan again returns the same identity, and any fork or skipped revision is refused |
| Region | A whole step, or a byte span `[start, end)` of its text on character boundaries |
| Annotation | A comment, a deletion (whole step only) or a suggested replacement on the latest revision, optionally blocking |
| Resolution | Names one unresolved blocking annotation and the latest revision it is resolved against |
| Decision | `accepted` or `changes_requested` on the latest revision. Acceptance is refused while any blocking annotation on any revision is unresolved |

Only a policy reviewer who is not the plan's author may annotate, resolve or
decide. A refused command leaves the review thread unchanged.

The gate for the latest revision is evaluated in this order:

1. `no_plan`;
2. `unresolved`, with the open blocking annotations;
3. `unreviewed`, `changes_requested` or `accepted`, from the last decision on
   the latest revision.

Blocking feedback added after acceptance closes the gate again.

## Admission

While a policy selects the work's repository, `WorkLedger` adds the block reason
`plan_not_accepted` unless the gate for the exact work identity is `accepted`.
Every claim path already requires ready work:

- `claim_next` takes only ranked work;
- fair admission marks work that is not ready as `denied`.

Accepting a plan therefore claims, reserves and dispatches nothing. A new
version of the work has a new identity and needs its own accepted plan.

## Feedback and comparison

`plan_feedback` returns the latest plan and its gate. It lists every annotation
with its revision, region, quoted text, note, blocking flag and resolving
revision. It also compares the latest revision with the plan it replaces, by
step id: kept, changed, added and removed steps, and whether kept steps were
reordered. Returning that feedback to an agent is the calling shell's job.

## Operator commands

```console
kiln-implementer plan-review --database DB --enrollment ENROLLMENT --command-file COMMAND
kiln-implementer plan-feedback --database DB --enrollment ENROLLMENT --work WORK_IDENTITY
kiln-implementer plan-review-receipt --cases fixtures/plan-review/cases.json
```

A command file holds one `PlanReviewCommand` as JSON: `configure`, `propose`,
`annotate`, `resolve` or `decide`. `plan-review` applies it in the same SQLite
transaction as the work ledger. All paths must be absolute.

## Frozen comparison

`config/plan-review-contracts.ncl` types a small set of cases, and
`config/fixtures/plan-review-positive.ncl` exports them to
`fixtures/plan-review/cases.json`. `scripts/check-nickel-fixtures.sh` refuses a
stale export and checks that a self-review case is rejected.
`plan_review_receipt` replays each case through the review rules, then
classifies it:

| Outcome | Meaning |
| --- | --- |
| `changed` | Review accepted a different plan than revision 1; the receipt carries the comparison |
| `prevented` | No plan was accepted, where accepting revision 1 would have admitted implementation |
| `unchanged` | Revision 1 was accepted as written |
| `unavailable` | No plan bytes (`missing_plan`) or no decision (`not_reviewed`); never a pass |

The committed expectation is `fixtures/plan-review/expected.json`. The frozen
cases have one of each of the first three outcomes and two unavailable cases.

## Non-claims

- The cases are synthetic plans written for this fixture. No captured agent
  plan is in the repository, so the captured case is unavailable, and the
  receipt measures no quality gain.
- Accepting revision 1 unchanged stands in for skim-and-approve; no reviewer
  behaviour was observed.
- Plan acceptance does not replace code review, typed check evidence (#35),
  preview comparison (#65), attempt fencing (#45) or canonical CI verification.
  It happens before them.
- Plannotator is not vendored or required.
