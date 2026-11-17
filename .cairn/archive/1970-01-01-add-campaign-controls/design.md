## Context

`kiln_implementer::campaign::CampaignStore` owns campaign state in the `implementation_campaign` table with version compare-and-swap. `campaign_operator::CampaignCommand` exposes start, inspect, cancel, continue, reconcile, issue decision and resolve decision through a command file. Each store method runs its own transaction. A retried command fails as stale, and nothing records who issued it. `CampaignState::add_decision` admits a request only when the member's current policy, work and candidate match it; `resolve_decision` checks only the request's own bindings and options.

The web service authenticates every `/api/` request into a `Principal { id, role }` (`viewer`, `operator`, `worker`). `api_admission` requires the configured Origin and JSON for POST and refuses viewers except on two command routes. `GET /api/implementation/campaigns?campaign=ID` reads the campaign projection from the `--implementation-work` database. `work.rs` shows the existing pattern for writes: a Nickel-checked profile lists exact write principals. The exploration example routes the same library handlers with a fixed `local:operator` principal. `implementation_campaign::delivery::route_supports_goal` already decides which completion goals each delivery route supports.

## Goals and Success Contract

An authorized operator can preview and start a configured campaign, continue, pause and cancel admission, and issue and answer decisions from the console. Every mutation is one canonical owner transaction. Retries reconcile, stale input conflicts, and authority is narrowed per principal, campaign, command and review role. Viewers read and workers see nothing.

## Decisions

### Decision: typed operator intents and a durable command record at the owner

**Choice:** `campaign::commands` adds `OperatorIntent` (`start { contract }`, `continue`, `pause`, `cancel`, `issue_decision`, `resolve_decision`, each with `campaign_id` and `expected_version` except `start`) and `CampaignStore::apply_operator(OperatorCommand { request_id, actor, intent })`. One immediate transaction does the following:

1. Read `implementation_campaign_command(request_id, campaign_id, fingerprint, outcome)`. An equal fingerprint returns the stored outcome as a replay; another fingerprint is `request_conflict`.
2. Apply the transition through the same in-transaction helpers the existing store methods now share. This keeps compare-and-swap, the shared parent-budget decision bound and the #87 wake hints (`review` on resolution, `operator` on continue).
3. Insert the outcome row and commit.

The fingerprint is BLAKE3 over the schema tag, the actor and the intent, so another principal cannot replay or hijack a request ID. Failures are not recorded; a retry recomputes them. Retention is bounded by `MAX_OPERATOR_COMMANDS`, and a full record refuses new commands rather than evicting history. Failures are typed: `stale` (expected and current version), `missing`, `request_conflict`, `refused { code }` (owner diagnostics such as `campaign_decision_stale`, `campaign_admission_closed`, `campaign_decision_queue_full`, `campaign_exists`), `retention_exhausted` and `storage`.

`CampaignCommand` gains `pause`. The command-file CLI keeps its current behavior.

**Rationale:** the owner's transaction is the only place where "applied once" can be decided, and the shell cannot lose an outcome between commit and response. Typed failures let the shell map conflicts without parsing messages.

### Decision: a decision answer must match the member's current scope

**Choice:** `CampaignState::decision_in_current_scope(request)` holds the rule `add_decision` already used: the member is observed, its policy matches, and any bound work equals the observed work and any bound candidate is in the observed evidence. `add_decision` uses it unchanged. `resolve_decision` additionally refuses with `campaign_decision_scope_changed` when it no longer holds.

**Rationale:** an answer to a review of a superseded candidate must not count for its successor. This applies to every caller, including the CLI.

### Decision: a re-read control profile is the receiving shell's authority

**Choice:** `serve --campaign-controls FILE` loads `kiln.web-campaign-controls.v1`, exported from `config/web-campaign-controls.ncl`:

- `campaign_database`: absolute path. The service requires `--implementation-work` and both must name the same canonical file, so the console controls what it displays.
- `grants`: 1 to 128, each `{ principal, campaigns (1 to 64), commands (subset of the six intents), review_roles (0 to 32), not_after_ms (optional) }`. Principals are exact and unique; wildcards are invalid.
- `definitions`: 0 to 64 of `{ contract: absolute path to a frozen CampaignContract JSON, delivery_route: github_pull_request | native_git }`, with unique campaign IDs.

The service validates the profile at startup and rereads it, bounded, for every control request. An unreadable or invalid profile refuses all controls with `campaign_controls_unavailable`. A mutation needs a grant for the principal that names the campaign and command and has not expired. A `required_review` answer also needs one of the member's `required_reviews` among the grant's `review_roles`.

**Rationale:** removing a grant or letting it expire takes effect on the next request without a restart. Holder-bound Basalt capabilities from #69/#70 can later replace the grant lookup without changing the intents or the owner record.

### Decision: start only frozen, previewed definitions

**Choice:** a definition's identity is its contract's selection identity (`CampaignContract::identity`). `GET /api/implementation/campaign-definitions` returns `kiln.campaign-definitions.v1`. Each preview has the identity, campaign ID, revision, repository, delivery route, the goals the route offers, and per member the change, tasks, prerequisite inclusion, workflow profile, profile, policy, repository-instruction and integration-policy identities, context entries, goal, whether the route supports the goal, and required checks and reviews. It also lists the concurrency and decision bounds, parent budget, agent definition, an existing campaign's version and whether its selection equals the definition, plus `can_start` and the reasons it is false. A start intent names only `definition_identity`. The shell rereads the definition, refuses `campaign_definition_changed` if its identity moved, `campaign_endpoint_unsupported` if a member goal is outside the route, and lets the owner refuse an existing campaign.

**Rationale:** the client never supplies a contract, path or executable. Scope expansion or a changed selection needs a new definition and the owner's explicit revision; it cannot join a running campaign.

### Decision: an inbox with typed decisions and the caller's open operations

**Choice:** `GET /api/implementation/campaigns/inbox?campaign=ID` returns `kiln.campaign-inbox.v1`:

- campaign ID, store version and campaign identity;
- admission state (`open`, `paused` or `cancelled`);
- the caller's open `operations`: granted commands that the current state admits;
- every decision with category, state, question, options, recommendation, member, bound work, policy and candidate identities, revision, priority, creation time, pending reason, resolved answer, `scope` (`current`, `changed` or `member_unobserved`), required review roles and the caller's `actions` (`issue` or `resolve`).

Outstanding decisions come first, then unissued, resolved and expired, each ordered by priority, creation time and identity. Viewers get an empty `operations` list and no actions. The route is GET-only and reads through `CampaignObservation`.

**Rationale:** the console renders answer forms only where the owner would admit them, and resolved decisions stay visible in scope.

### Decision: command route and response codes

**Choice:** `POST /api/implementation/campaigns/commands` takes `{ request_id, command, ... }` with unknown fields denied. Request IDs are 8 to 128 characters from `[A-Za-z0-9._:-]`. Success returns `kiln.campaign-command-result.v1` with `request_id`, `replayed`, `command`, `campaign_id`, the post-transition `store_version`, `admission`, the affected decision's `{ identity, state }` and non-claims.

| Status | Codes |
| --- | --- |
| 400 | `campaign_command_invalid` |
| 403 | `campaign_controls_forbidden` (worker), `campaign_command_denied` (no, narrower or expired grant, or wrong review audience) |
| 404 | `campaign_not_found`, `campaign_decision_unknown` |
| 409 | `campaign_version_conflict`, `campaign_request_conflict`, `campaign_definition_changed`, `campaign_endpoint_unsupported`, `campaign_exists`, `campaign_decision_stale`, `campaign_decision_scope_changed`, `campaign_admission_closed`, `campaign_decision_queue_full` |
| 503 | `campaign_controls_unavailable`, `campaign_controls_not_configured`, `campaign_command_retention_exhausted`, `campaign_store_unavailable` |

The server records every 403 in the existing denial audit.

**Rationale:** a 409 tells the console to reload state; nothing was applied. A 403 never reaches the owner.

### Decision: admission controls say what they do not do

**Choice:** pause stops successor admission and decision issuance, continue reopens it, and cancel closes the campaign permanently. Results, the inbox and the console state that none terminates running attempts or workers, cleans up provider resources or refunds usage. Retained effects still reconcile through the driver.

### Decision: console and exploration

**Choice:** `inventory.js` loads the inbox beside the projection and renders the following:

- admission controls (cancel requires an explicit confirmation);
- a decision inbox with an answer form for each decision whose `actions` contain `resolve`, and an issue button for `issue`;
- a start-preview list with a start button where `can_start` is true.

Each submission sends the inbox's `store_version`, a fresh `crypto.randomUUID()` request ID and disables its controls until done. After a lost response it offers a retry that resends the same request ID. A 409 reloads state and reports that nothing was applied. The exploration workspace writes a control profile granting `local:operator` over its synthetic campaign and two synthetic definitions (one startable, one with an endpoint its route does not support), and seeds two outstanding decisions and one unissued decision without changing the proposed next action.

## Risks / Trade-offs

- Profile grants are principal lists, not holder-bound capabilities; revocation is a profile edit. #69/#70 own stronger authority.
- The command record is never pruned; at its bound, commands stop until an operator archives it.
- Rereading the profile per request adds one bounded file read to each control request.
- The scope check on resolution may refuse answers that were acceptable before for callers that changed the member between creation and answer; that is the intended correction.
