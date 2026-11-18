# Plan review verification

Issue: <https://github.com/OnixResearch/kiln/issues/93>.

## Delivery and ownership

The immediate result is an opt-in plan-review gate in front of implementation
admission. Without an operator-configured policy, admission and the stored work
ledger are unchanged. The durable capability has four parts:

- `kiln_core::implementation::plan_review`: pinned plans, regions,
  annotations, resolutions, decisions, the plan gate, feedback, revision
  comparison and the replayed comparison receipt;
- `WorkLedger::apply_plan_review` and the `plan_not_accepted` block reason;
- `Implementer::plan_review` and `plan_feedback`, and the `kiln-implementer
  plan-review`, `plan-feedback` and `plan-review-receipt` commands;
- the Nickel case contract, its checked export and the committed receipt
  expectation `fixtures/plan-review/expected.json`.

Kiln implementation-work maintainers own it. The Cairn workspace names the new
block reason (`plan_not_accepted`, operation `request_decision`) so its exhaustive
projection stays complete.

## Exercised checks

Commands ran from this checkout through `nix develop path:$PWD -c`.

| Check | Observed result |
| --- | --- |
| `cairn gate proposal\|design\|tasks review-implementation-plans ...` | PASS before implementation, and PASS again after the tasks were ticked |
| `cargo test --locked -p onix-kiln-core --lib plan_review` | 7 passed. The first run failed 4 tests because their fixture identities used non-hex characters; the identities were corrected |
| `cargo test --locked -p onix-kiln-core --lib` | 370 passed |
| `cargo test --locked -p kiln-implementer` | All suites passed: 47 library tests (6 ignored), and 16, 18 and 2 integration tests, including the new durable test |
| `cargo test --locked -p kiln-adapter-github --test implementer_cli` | 2 passed, including the new plan-review CLI test |
| `cargo test --locked -p kiln-web-service --lib cairn_workspace` | 12 passed with the new `plan_not_accepted` blocker projection |
| `cargo clippy --locked -p onix-kiln-core -p kiln-implementer -p kiln-adapter-github --all-targets -- -D warnings` | PASS |
| `cargo clippy --locked -p kiln-web-service --all-targets -- -D warnings` | PASS |
| `bash scripts/check-nickel-fixtures.sh` | PASS (93 positive, 618 negative), including the new positive export's freshness and the refused self-review negative |
| `kiln-implementer plan-review-receipt --cases fixtures/plan-review/cases.json` | Receipt for cases `ced17f4d7eda99997dd27bd4ca1a57b1d4ad4ebdcea14f5cf2be56e67c9e9a7d`: 1 changed, 1 prevented, 1 unchanged, 2 unavailable; committed as the expectation |
| Scoped `rustfmt --check --edition 2024` | PASS for the touched Rust files |
| `cairn sync review-implementation-plans ... --execute` | Added `.cairn/specs/implementation-plan-review/spec.md`; nothing was archived |
| `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | Valid, 0 issues |
| `cairn traceability coverage ... --json` | 1,903 requirements, 1,901 referenced. It still exits 1 with the same two missing GitHub anchors and 71 unrelated dangling IDs. All 7 new plan-review IDs are referenced |

The tests pair positive and negative cases:

- **Plans:** an identical re-proposal is idempotent and a linear revision is
  pinned. A fork, a skipped revision, a wrong previous plan, another work
  identity, a first revision with a previous plan, duplicate or blank or
  oversized steps and a wrong schema are refused, and the thread is unchanged.
- **Annotations:** a whole-step comment, a span replacement and a step deletion
  are accepted. A duplicate id, a missing step, a span past the end, an empty
  span, a partial deletion, a blank comment or replacement, an unknown plan,
  the plan's author, a reviewer outside the policy, a span that splits a
  character, and an annotation on an older revision are refused.
- **Gate:** no plan, unresolved blocking annotations, unreviewed,
  changes-requested and accepted states; acceptance with open blocking
  feedback, a stale or self-made resolution, a second resolution and acceptance
  of an older revision are refused; blocking feedback after acceptance closes
  the gate again.
- **Feedback and comparison:** quoted regions, resolving revisions, kept,
  changed, added and removed steps, and reordering.
- **Default:** no policy, or a policy for another repository, keeps work ready
  and ranked, refuses other commands, and keeps the stored ledger byte-identical
  after the policy is cleared. A malformed policy is refused.
- **Opt-in:** selected work is blocked with `plan_not_accepted` until its exact
  version has an accepted plan, the author cannot accept it, and a superseding
  version needs its own plan. The durable test proves `claim_next` returns
  nothing before acceptance, acceptance survives a reopen without creating an
  attempt, and the claim then succeeds. The CLI test drives configure, a refused
  self-review, annotation, feedback, a revision, resolution, acceptance and the
  claim.
- **Receipt:** the frozen cases match the committed expectation. A wrong schema,
  no cases, a duplicate case, a decision past the revisions and a self-review
  reviewer are refused.

## Boundaries

- The frozen cases are synthetic plans in the style of an agent's plan for
  Kiln stack publication. No captured agent plan is in the repository; the
  captured case is unavailable, and no quality gain is measured.
- No UI, planning-job dispatch or agent feedback transport was added.
- The default admission policy did not change.
- The full workspace tests, Octet, release-readiness and flake checks were not
  run.
