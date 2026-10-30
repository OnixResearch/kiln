## Context

- Hosted pipelines (`crates/kiln-hosted/src/pipelines.rs`) compile `kiln.hosted.pipeline.v1` into a BLAKE3-identified plan. `Trigger` is the bare enum `manual | push | pull_request`, serialized as strings inside the plan, so its bytes are part of every plan identity. Hosted execution (`execution.rs`) runs a reviewed plan on an explicit customer request; nothing evaluates triggers today.
- `migrate` imports a fail-closed subset of GitHub Actions and rejects any non-empty trigger configuration.
- Web automation (`automation.rs`, `engine/automation.rs`) admits signed GitHub push and pull-request events for one exact registered reference. It reserves one source record per receiving registration plus one sibling per other project. It consults the monorepo shell (`monorepo::evaluate`) only when a project is scoped. Source records are durable journal state, and the policy fingerprint is BLAKE3 over the serialized profile.
- The GitHub adapter (`kiln-adapter-github/src/events.rs`) admits `opened`, `reopened` and `synchronize` pull requests and drops closed ones.
- Review (`kiln_core::change_review::checklist`) requires the latest full, clean, successful run for each required project at the exact commit. `ci_required` comes from the monorepo affected decision.
- `kiln_core::paths::matches` is the shared glob matcher (`*` and `?` within a segment, whole-segment `**`). `kiln_core::monorepo::ChangedFiles` is the exact-or-fallback changed-file fact. `kiln-core` is `no_std` with `alloc`.

## Goals and Success Contract

Given a trigger filter, an event and exact-or-unknown changed files, Kiln decides deterministically whether the trigger runs. It records why (matched reasons) or why not (the one excluding filter). Unknown changed files never cause a skip. No filter means exactly today's behaviour. A filtered pipeline never satisfies required CI unless the review policy explicitly declares that check optional for the changed paths.

False completion includes:
- an existing pipeline whose plan identity changes;
- an automation profile whose fingerprint changes;
- a filtered event that still queues a run;
- a run without its matched trigger reason;
- a skip caused by unknown changed files;
- a required check silently waived by a filter;
- an Actions `*-ignore` filter silently dropped;
- glob semantics that differ from review ownership and monorepo scopes.

## Decisions

### Decision: One pure filter in kiln-core

**Choice:** `kiln_core::trigger_filter` defines:
- `TriggerFilter { branches, tags, types, paths }`, each serde-default and omitted when empty, with `deny_unknown_fields`;
- `PullRequestType` (`opened`, `reopened`, `synchronize`, `closed`, `merged`);
- `EventKind` (`manual`, `push`, `pull_request`, `schedule`);
- `TriggerEvent`;
- `admit_filter(kind, filter)`;
- `evaluate(kind, filter, event, changed) -> FilterDecision`.

`FilterDecision` is either `Run { reasons }` or `Skip { excluded_by }`, where `excluded_by` is one of `event`, `branches`, `tags`, `types` or `paths`.

Bounds are named: `MAX_FILTER_PATTERNS = 32` per list and `MAX_FILTER_PATTERN_BYTES = 256`. Patterns must satisfy `paths::valid_pattern`, so `[]`, `{}`, `!` and relative segments are rejected. Changed paths are bounded by the existing monorepo bounds before they become exact facts.

**Rationale:** both hosted and web need identical semantics. The core already owns the glob matcher and changed-file facts; it has no I/O and stays `no_std`.

### Decision: Filter semantics

**Choice:**

| Field | Event kinds | Matches against | Empty means |
| --- | --- | --- | --- |
| `branches` | push, pull_request | push: branch name after `refs/heads/`; pull request: base (target) branch | every branch |
| `tags` | push only | tag name after `refs/tags/` | every tag |
| `types` | pull_request only | the pull request action; `merged` is a closed-and-merged pull request, `closed` is closed without merge | `opened`, `reopened`, `synchronize` (today's behaviour) |
| `paths` | push, pull_request | any changed path, including the old path of a rename | no path condition |

- For push, a filter with only `tags` excludes branch pushes, and a filter with only `branches` excludes tag pushes. A filter with both matches either. This is the GitHub Actions rule.
- Evaluation order: event kind, then ref (`branches`/`tags`), then `types`, then `paths`. The first failing filter is the recorded exclusion.
- `paths` with `ChangedFiles::Fallback(reason)` runs, with reason `paths_unknown { fallback }`.
- `manual` and `schedule` admit no filter fields.

### Decision: Hosted wire format preserves plan identity

**Choice:** `Trigger` becomes `Manual | Push(TriggerFilter) | PullRequest(TriggerFilter) | Schedule { schedule }`. It has hand-written serde.
- **Serialize:** an unfiltered trigger is the bare string (`"push"`), so existing plans keep identical bytes. Otherwise it is an object `{ "event": "push", "branches": [...] }`.
- **Deserialize:** accepts the bare string, or an object with `deny_unknown_fields`.
- **Compile:** admits each filter for its kind and allows at most `MAX_TRIGGERS = 8` triggers. Duplicate `(event, schedule)` keys are rejected. Two filtered triggers of one kind are rejected because the first match would hide the second.
- **Matching:** `pipelines::select_trigger(plan, event, changed)` returns the first matching trigger index with its reasons, or every trigger's exclusion.

**Alternatives considered:** a tagged object for every trigger was rejected because it would change every existing plan identity.

### Decision: Hosted run and non-run records

**Choice:** `PipelineRequest` gains an optional `event`, which is omitted when absent so that request identities are unchanged:
- `kind`;
- `reference`;
- `action`;
- `base_branch`;
- `base_revision`;
- `revision`.

With an event, `run_pipeline` computes changed files from the customer workspace repository through `kiln_web_service::monorepo::changed_files_at`. It returns `read_error` or `no_base` fallbacks when the facts are unavailable. If no trigger matches, it durably inserts the attempt with state `skipped` and a result naming each trigger's excluding filter. It reserves no finance and starts no process. A matched run adds `trigger: { index, event, reasons }` to its result. Without an event, behaviour is unchanged.

### Decision: Web automation filters

**Choice:** `TriggerProfile` gains `branches`, `tags`, `types` and `paths`, each serde-default and omitted when empty, so existing fingerprints are unchanged.

Admission rules:
- `tags` are rejected, because registrations bind `refs/heads` only.
- A push filter must match its bound reference.
- A pull-request trigger needs an exact `pull_request_base` or non-empty `branches`.
- `types` containing `closed` or `merged` asks the adapter to admit closed pull requests for that registration.

The receiving registration and its siblings use one selection predicate. The predicate checks:
- event kind enabled;
- exact head reference;
- base equal to `pull_request_base` or matched by `branches`;
- `types`;
- `paths`.

Changed files are computed once per event from the existing base rules (previous head for push, merge base for pull request) through `monorepo::changed_files`. They are shared with `monorepo::evaluate_changed`, so the diff is not repeated. A member excluded by its filter is recorded with status `filtered` and `excluded_by`. A queued member records `trigger.reasons`.

### Decision: GitHub pull request action

**Choice:** `VerifiedEvent.pull_request_action` is serde-default and omitted when absent. `admit_event` is unchanged (closed pull requests are still ignored). `admit_event_with(.., EventAdmission { closed_pull_requests: true })` admits `closed` events as `closed` or `merged`. The candidate identity of closed and merged events adds the action, so it never replays the synchronize identity of the same head. Existing identities are unchanged.

### Decision: Required checks stay required

**Choice:** filters never change `ci_required`, so a filtered pipeline leaves its required check without a run, and the checklist stays unmet.

The review `Policy` gains `optional_ci: [{ project, paths }]`, which is serde-default and omitted when empty. A required check for `project` with no qualifying run at the exact commit is satisfied only when the review's changed files are complete and non-empty, and every path and old path matches one of `paths`. A run that exists and failed still blocks. Unknown changed files never waive.

### Decision: Actions migration mapping

**Choice:** `migrate` maps the Actions filters on `push`, `pull_request` and `workflow_dispatch` (no options) as follows:

| Actions filter | Native mapping |
| --- | --- |
| `branches` | `branches` |
| `tags` | `tags` |
| `paths` | `paths` |
| `types` (`opened`, `reopened`, `synchronize`) | the same native types |
| `types: closed` | `closed` and `merged` |

Rejected with an explicit error:
- `branches-ignore`, `tags-ignore` and `paths-ignore`;
- `!` negation;
- `[` character classes;
- `+` (Actions has no single-character equivalent);
- unsupported types.

Every explicit mapping is recorded in diagnostics. The result compiles, so `migrate` round-trips into the same native trigger objects.

### Decision: Monorepo scopes and trigger paths

**Choice:** project scopes keep deciding which projects are affected. Trigger `paths` is a second, per-trigger filter. A run requires the project to be affected (or unscoped) and its trigger filter to match. An affected-but-filtered project is `filtered`; a scope-skipped project stays `skipped`.

## Risks / Trade-offs

- Hosted events and changed files describe a customer workspace. They decide whether a hosted plan executes, not whether review CI passes.
- Web registrations still bind one head reference. `branches` mostly widens pull-request bases and documents intent for pushes.
- Admitting closed pull requests runs CI on the closed head revision. It is opt-in per registration through `types`.
- Exclusion records consume `max_records` like skipped records.

## Verification Strategy

- Unit tests in `kiln_core::trigger_filter` for every field: match, non-match, the order of exclusions, unknown changed files running, rename old paths, and bound and pattern rejection.
- Hosted tests:
  - bare triggers keep the prior plan identity bytes;
  - filtered objects compile;
  - invalid filters are rejected;
  - `select_trigger` matches or excludes;
  - `migrate` round-trips `branches`, `paths` and `types`, and rejects `*-ignore`;
  - `run_pipeline` records a `skipped` attempt without reserving finance.
- Web engine tests:
  - path match and non-match;
  - type exclusion;
  - base glob;
  - unknown changed files running;
  - existing profile fingerprint and source record bytes unchanged.
- `kiln-core` review tests: a filtered required check stays unmet; `optional_ci` waives it only for covered exact paths; a failed run still blocks.
- Adapter tests: closed and merged admission is opt-in, and identities are distinct.
- Nickel positive and negative fixtures for the hosted pipeline, web automation and review policy.
