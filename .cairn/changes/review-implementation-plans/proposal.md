## Why

Issue [#93](https://github.com/OnixResearch/kiln/issues/93) asks for a default-off review of an agent's plan before an implementation attempt is admitted, in the style of [Plannotator](https://github.com/backnotprop/plannotator): the plan is a first-class artifact, reviewers annotate it, a revised plan is compared with the previous one, and implementation waits for an accepted plan. Kiln's review machinery today covers admitted changes and evidence (#35, #65) and attempt fencing (#45), not the plan that precedes an attempt.

## What Changes

- **Core (pure, `kiln_core::implementation::plan_review`):**
  - A plan artifact pinned by BLAKE3 over its steps and author, bound to one exact work identity, with numbered revisions that name the plan they replace.
  - Annotations on a named region (a whole step or a byte span), as a comment, a step deletion or a suggested replacement, optionally blocking, plus explicit resolutions and accept or changes-requested decisions.
  - Structured feedback for the author, and a step-level comparison of a revision with the plan it replaces.
  - A plan gate, and an opt-in policy that names repositories and reviewers.
- **Work ledger:** the policy and per-work review threads live in the work ledger. While a policy selects a repository, work without an accepted latest plan is blocked with a new block reason, so every claim path that requires ready work honours it. Without a policy, the ledger and its stored form are unchanged.
- **Durable store and operator CLI:** one transactional plan-review command on `Implementer`, and `kiln-implementer plan-review` and `plan-feedback` commands.
- **Frozen comparison:** a Nickel-authored set of plan-review cases, its checked JSON export, and a deterministic receipt with a committed expectation.
- **Documentation:** a plan-review guide, the documentation index and the changelog.

## Impact

Consumers: operators who want plan review for selected repositories, and later campaign or worker shells that return feedback to an agent. Owners: Kiln implementation-work maintainers. Files: a new core module and tests, the work ledger, the implementer store and CLI, the Cairn workspace blocker projection and console vocabulary for the new block reason, a Nickel contract and fixtures, a JSON export and expectation, the check script, the guides and the changelog.

## Non-goals

This change does not vendor Plannotator or add an external review UI, and it does not change the default admission policy. Plan acceptance does not replace code review, #35's typed check evidence, #65's preview comparison, #45's attempt fencing or canonical CI verification. It adds no web surface and does not dispatch planning jobs to agents. The frozen cases are synthetic; no captured agent plan exists in this repository, so no measured quality gain is claimed.
