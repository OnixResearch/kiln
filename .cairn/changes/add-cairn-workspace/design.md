## Context

The implementation database (`--implementation-work`) holds the canonical work ledger (`work`), the attempt ledger (`attempts`), native lifecycle receipts (`native-git-lifecycle:<attempt>`) in `implementation_state`, and selected campaigns in `implementation_campaign`. Canonical work names Cairn units through aliases of kind `cairn_task` or `cairn_artifact` whose native ID is `<change>/<unit>` and whose version is the admitted readiness snapshot. Campaign contracts select a whole change (`<change>/*`) or tasks within it and observe each member's stage, enumeration, blockers, remaining native units and canonical work binding. Attempts carry their phase, fence, candidate, handoff, verification and publication records. The campaign projection hides the raw repository binding behind its BLAKE3 digest.

## Goals and Success Contract

One read-only view per Cairn change joins every existing observation by exact identity. It keeps blocked, planning-only, completion-only and incomplete members visible and separates readiness, execution, evidence applicability, integration and lifecycle. Blockers carry a structured condition, their supporting observation and the owner's proposed operation, and historical evidence never reads as current.

## Decisions

### Decision: one bounded read transaction in the implementer

**Choice:** `ImplementationObservation::snapshot()` reads, in one deferred read transaction on the read-only, query-only connection:
- the `work` and `attempts` records, each absent when never written;
- at most `MAX_SNAPSHOT_LIFECYCLE_RECEIPTS` native lifecycle receipts;
- campaigns in identity order until `MAX_SNAPSHOT_CAMPAIGN_BYTES` total.

Truncation is reported, not hidden. A missing table counts as no records. An oversized or malformed record fails as malformed.

**Rationale:** the storage format belongs to the implementer. One transaction keeps the join consistent across tables without escalating a GET into a writer.

### Decision: key by repository identity and change name

**Choice:** a Cairn change is `(repository_identity, change)`, where `repository_identity` is BLAKE3 of the canonical repository string, from the one `repository_identity` function the campaign projection also uses. Units come from:
- canonical aliases (`cairn_task` or `cairn_artifact`) split at the first `/`;
- campaign task selectors, `<change>/<task>`; a whole-change selection (`<change>/*`) names the change, not a unit;
- campaign remaining-obligation units.

Each unit records which sources named it.

**Rationale:** canonical work, campaigns and projects then resolve the same exact identity without exposing a raw repository path. Project names come from canonical work.

### Decision: separate stage facets with explicit unknown meanings

**Choice:** each unit reports five facets:

| Facet | Values |
| --- | --- |
| `readiness` | `ready`, `blocked`, `unknown` (eligibility unavailable), `not_admitted` (from canonical eligibility; `ready` only when every current version is ready) |
| `execution` | phase of the latest attempt on current work, or `not_started` |
| `evidence` | `current_verified`, `current_failed`, `historical_only`, `none` |
| `integration` | publication phase of that attempt, or `not_published` |
| `lifecycle` | `receipt_observed` for current work, or `not_observed` |

An attempt's evidence is `current` only when its work identity equals the unit's current canonical work identity; otherwise it is `historical`. A lifecycle receipt counts only when it binds the attempt's own work identity and fence; otherwise it reads as `crossed`. An attempt's dispatch is `not_reserved`, `reserved` or `transport_started`, and an unknown provider outcome is `unreconciled` or `reconciled`. Campaign member work bindings are `current`, `stale` (a superseded canonical version), `unknown_work` (not in the ledger) or `not_bound`.

**Rationale:** these are owner facts with different authority. A verified historical attempt cannot make changed work look accepted, and an unknown binding is not absence.

### Decision: structured blockers, no prose parsing

**Choice:** a blocker is `{ condition, subject, observation, operation, owner, detail }`. The subject is a native unit or campaign member, the operation is a campaign operation, and owners use the campaign owner vocabulary (`CampaignOperation::owner`). Sources:

| Source | Conditions | Operation and owner |
| --- | --- | --- |
| Canonical eligibility | `missing_dependency`; `no_acceptance_obligations`, `dependency_cycle` | `wait`; `blocked`, by `native-source-owner` |
| Campaign holds | `campaign_hold` | The hold's own operation and owner, else the operation's owner; its reason is displayed verbatim |
| Unapplied campaign effects | `pending_effect` | `reconcile` by the effect's owner |
| Unreconciled unknown dispatch outcomes (`AttemptLedger::unresolved`) | `dispatch_recovery` | `reconcile` by `attempt-reconciliation-owner` |
| Stale or unknown member bindings | `stale_work_binding` | `replan` by `campaign-policy` |

Displayed reasons are never parsed into decisions. A transport that merely started is not a blocker: it is normal while an attempt executes, and the canonical barrier is the unreconciled unknown outcome that refuses another claim.

**Rationale:** owners already emit these structured facts. Prose stays display data.

### Decision: read-only routes and a URL-bound console page

**Choice:** `GET /api/implementation/cairn` returns `kiln.cairn-workspace-index.v1` and `GET /api/implementation/cairn/detail?repository=HEX&change=NAME` returns `kiln.cairn-workspace-detail.v1`. Both are GET-only, refuse worker principals, admit only the exact query shape and are bounded to 4 MiB. The pure projection lives in `cairn_workspace.rs`; admission, the snapshot read and the byte bound live in its `http` shell, which the service and the exploration workspace share. The console page `cairn` (2.7, under Work) keeps `cairn_repository`, `cairn_change` and `cairn_q` in the URL. A generation counter drops a late response, and the detail response must echo the requested key. The selected-campaign view links each selected change to its detail from a navigation list beside the projection; the projection itself stays inert, without links or buttons.

**Rationale:** reload reproduces the binding, and the page follows the existing register, panel and module pattern.

### Decision: exploration example over the same synthetic database

**Choice:** the exploration workspace admits canonical Cairn task work into its synthetic campaign database, as follows:

| Member | Canonical work and attempts |
| --- | --- |
| A, `synthetic-planning/task-plan` | One failed attempt |
| B, `synthetic-assurance/task-review` | Depends on A's work identity |
| C, `synthetic-ready/task-build` | Revision 1 claimed, then superseded by revision 2 with a new claim; the campaign member still binds revision 1 |
| D, `synthetic-lifecycle` | Completion-only, campaign selection only |

The existing review decisions supply the review wait.

**Rationale:** the example exercises the real stores and the issue's A→B, C and D scenario without contacting any provider.

## Risks / Trade-offs

- Every authenticated viewer or operator sees every change, as for campaign projections; capability-restricted observers wait for #69/#70/#75/#76.
- Requirement text is limited to canonical acceptance obligations until typed Cairn snapshots (#83) supply native requirement and acceptance-criterion references.
- The index decodes campaigns and ledgers on every request; the bounds keep it finite, and caching is not added.
- Canonical work keys hash the alias set without the repository, and Cairn aliases are `<change>/<unit>`. Two enrolled repositories naming the same Cairn unit therefore collide in the work ledger (refused as ambiguous, or superseded by the higher revision). The workspace keys changes by repository, but it cannot show work the ledger refused or replaced. This predates the change and is reported separately.
