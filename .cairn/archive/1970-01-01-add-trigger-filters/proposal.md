## Why

Every push and pull request runs every hosted pipeline and every web automation trigger, and migrated GitHub Actions workflows that use `branches`, `tags`, `paths` or `types` cannot be expressed at all (issue #13). Hosted `Trigger` is a bare `manual`/`push`/`pull_request` enum, `migrate` rejects every trigger option, and web automation triggers only bind one exact reference with `push`/`pull_request` switches.

## What Changes

- **Pure filter core:** a new `kiln_core::trigger_filter` module defines a bounded `TriggerFilter` (`branches`, `tags`, `types`, `paths`), admission per event kind, and a deterministic `evaluate` that returns either the matched reasons or the one filter that excluded the event. Globs reuse `kiln_core::paths::matches`; changed files reuse `kiln_core::monorepo::ChangedFiles`. Unknown changed files match (fail toward running).
- **Hosted pipelines (`kiln.hosted.pipeline.v1`):** a `triggers` entry may stay a bare string or become an object `{ event, branches?, tags?, types?, paths? }`. Bare strings keep their exact serialized bytes, so existing plan identities are unchanged. `execution.run` accepts an optional source `event`; it records the matched trigger and reasons on the run, or durably records a `skipped` non-run with the excluding filter.
- **Web automation (`kiln.web-automation.v1`):** triggers accept the same optional filter fields. Pull requests may admit any base matched by `branches` instead of one exact `pull_request_base`. Changed files are computed once from the exact base and candidate through the monorepo shell. Each source record keeps the matched trigger and reasons, or a `filtered` status with the excluding filter.
- **GitHub event admission:** a pull request's action is retained, and closed/merged pull requests are admitted only for registrations that ask for them.
- **Review policy:** an explicit `optional_ci` rule lets a required check be absent only when every exactly known changed file lies in the declared paths. Otherwise a filtered pipeline keeps the submit checklist unmet.
- **Migration:** `migrate` maps Actions `branches`, `tags`, `paths` and `types`, maps `closed` to `closed` plus `merged`, and rejects `*-ignore`, negated and character-class patterns with explicit errors.
- **Configuration and docs:** Nickel contracts for hosted pipelines, web automation and code review, positive and negative fixtures, operator and hosted CI documentation.

## Impact

- **Consumers:** operators and repository owners who author hosted pipelines or web automation, reviewers whose required CI may be filtered, and the Tangled `when` mapping (#14).
- **Owners:** Kiln maintainers own `kiln_core::trigger_filter`; hosted maintainers own the pipeline schema, migration and execution record; web-service maintainers own the automation shell and review policy wiring.
- **Compatibility:** all new fields are serde-default and omitted when empty. Existing pipelines, plans, automation profiles, policy fingerprints, source records and review policies serialize byte-identically.

## Out of Scope

- Actions `branches-ignore`, `tags-ignore`, `paths-ignore`, negated (`!`) patterns and character classes; they stay rejected.
- Event kinds other than push, pull request, manual and schedule (schedule is #16).
- Re-evaluating filters for manual reruns; a manual run is never filtered.
- Changing how monorepo project scopes decide affected projects; trigger `paths` is an additional per-trigger filter.
