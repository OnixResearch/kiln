## Context

- Branch intake lives in `crates/kiln-web-service/src/code_review/intake.rs`: `plan()` resolves an admitted `refs/heads/…` source into `Plan::{Create,Upload,Replay}`, keyed by `State.contributions[source_ref]` with a deterministic BLAKE3 change id. `commands.rs` lowers the plan into the existing `Create`/`Upload` actions, reserves one provider `Register` operation per request in the durable journal, and only records `Contribution.commit` on a successful completion.
- `kiln_core::source::JujutsuFacts` + `admit_jujutsu_facts` already refuse a non-Git backend, a conflicted bookmark, a divergent change and an ambiguous resolution. `kiln-adapter-jj` has a port-based `resolve_and_acquire` but no concrete jj reader.
- `kiln_core::change_review::Dependency { change, revision }` binds a dependency to an exact revision; readiness requires the dependency's current revision. `Command::SetDependencies` is owner-only, open-only and cycle-checked.
- jj 0.45 (dev shell) writes the `change-id` header into Git commit objects of colocated repositories and keeps `refs/jj/keep/*`; `json(...)` templates, `evolog` entries with `predecessors` and `operation` are available.
- `queue.rs` already re-enters `Workspace::command` with derived request ids, which is the precedent for orchestrating several journaled commands under one request.

## Goals and Success Contract

Given an exact jj change ID (or an ordered stack of them, or a pushed branch with `change-id` headers) from an authenticated write principal, Kiln resolves each ID to exactly one commit visible at exactly one observed jj operation head, admits it through `JujutsuFacts`, and maps it to exactly one Kiln change whose revisions are the distinct commits observed for that ID, in order.

Success means:
- one change rewritten three times and submitted after each version gives one Kiln change with three revisions, and revision N's recorded jj predecessors contain revision N-1's commit;
- divergent, conflicted (bookmark or commit), operation-less/divergent-operation, unreachable and malformed inputs are refused with stable `review_jj_*` codes and leave the journal, state and provider untouched;
- a duplicate submission appends nothing;
- a 4-change stack creates 4 changes with a 3-edge dependency chain; replay creates nothing; editing 2 changes appends exactly 2 revisions; a reorder rewrites the edges; a removed ID is flagged on its change and the change stays open;
- the jj repository's operation heads, operation store, refs and working copy are byte-identical before and after every Kiln observation.

False completion includes: a revset or prefix accepted as a key; an effect issued before the whole stack is admitted; a removed change abandoned; a jj observation used as a Kiln identity; a duplicate revision on replay; a new field serialized when default.

## Decisions

### Decision: Read-only jj observation shell in `kiln-adapter-jj`

**Choice:** a `cli` module in `kiln-adapter-jj` reads the operation head from `.jj/repo/op_heads/heads` directly (filesystem read; exactly one 128-hex head admitted), checks `store/type == git` and resolves `store/git_target`. It then runs the configured absolute jj executable through `bounded-exec` with a cleared environment (`JJ_CONFIG=/dev/null`), `--repository <workspace> --ignore-working-copy --at-op <head> --no-pager --color never`, and fixed JSON templates: one `log` over `change_id(<id>) | …` built only from validated full IDs, then one `evolog --limit 1 -r <commit>` per resolved commit. Parsing and fact construction are pure functions. Named limits bound stdout, stderr, time and stack size.

**Rationale:** `--at-op` never merges operation heads, `--ignore-working-copy` never snapshots, and reading op heads from disk avoids the one jj command path that would write. A direct jj-lib dependency would pin a moving library into the workspace.

**Alternative:** implement `SourceRepositoryPort` and reuse `resolve_and_acquire`. Rejected: that path acquires a canonical manifest the review provider already owns and accepts symbolic references.

### Decision: Admission through `JujutsuFacts`, with one new fact

**Choice:** each resolved change builds `JujutsuFacts { backend, operation_identity, change_id, resolved_commits, conflicted_bookmark, divergent_change, conflicted_commit }` and goes through `admit_jujutsu_facts`. `conflicted_commit` is a new serde-default fact omitted when false, so existing serialized facts and identities are unchanged. Core diagnostics map to `review_jj_<code>` API errors. The shell additionally requires the commit to exist in the review provider's Git object store with a matching `change-id` header when present, the jj Git store to canonicalize to the provider repository, and stack members to be in ancestor order.

**Full-ID rule:** `kiln_core::source::valid_jujutsu_change_id` admits exactly 32 lowercase `k..=z` characters (jj reverse-hex of a 16-byte Git-backend change id) and rejects the all-`z` root change. Prefixes and revsets are never accepted.

### Decision: Mapping key and identity

**Choice:** a jj contribution reuses `State.contributions` under the key `jj:<change id>` (branch keys always start with `refs/heads/`, so namespaces cannot collide) and the Kiln change id is `BLAKE3(["kiln.review.jj-change.v1", repository_id, change_id])`. The header path and the jj path share the key, so a jj user on any Git forge gets the same mapping. The contribution binds principal and target exactly like branch intake; a change ID grants no write authority.

### Decision: Orchestrated, resumable multi-change intake

**Choice:** a new `jj_contribute` action is handled by `jj_intake::contribute`. It observes every member, runs the pure planner over all members, and refuses the whole request before any provider effect on any failure. It then re-enters `Workspace::command` per member with derived request ids (`BLAKE3([kind, request_id, change id, commit|edges])`): a `skip_deserializing` internal `JjMember` action lowered into the existing `Create`/`Upload` path (so journaling, reservation, recovery and provider completion are reused), followed by `SetDependencies` where the edge differs. The parent request's final response is journaled last, so a retried parent request replays completed members and resumes the rest.

**Rationale:** each provider effect keeps its own durable reservation; no new effect kinds.

### Decision: Stack semantics

**Choice:** `stack: true` requests own the dependency edges of their members: the bottom member has none, member *i* depends on member *i−1* at that member's current revision after this request's uploads. Members whose change is `submitted` are left untouched; an `abandoned` member is refused. A stack has a BLAKE3 id derived from the first submitted bottom change and keeps its ordered members; on resubmit it is found through the lowest member already in a stack. Members absent from the new list are added to the stack's `removed` list and reported on their change detail; reappearing members are cleared. Single-change requests (`stack: false`) never touch stack membership or dependencies.

### Decision: Provider observations stored beside, not in, the domain

**Choice:** `State.jj` (omitted when empty) holds `revisions: {kiln revision id → RevisionObservation}` and `stacks`. A `RevisionObservation` records `source` (`jj` or `git_change_id_header`), `change_id`, `commit`, the observed `operation`, the creating `created_by_operation`, jj `predecessors`, and the Git author name/email/time. It is written after the member's revision registers and back-filled on replay. `kiln_core::change_review::Revision` is unchanged.

### Decision: Evolog view and published mapping

**Choice:** change detail gets `jj_evolog` (one row per revision with predecessors, operation, author, declared authorship, `predecessor_matches_previous` and `interdiff: "<prev revision id>..<revision id>"`) and `jj_stack` (stack id, position, removed flag). The console renders it as an "Evolog" section linking `review_interdiff=<from>..<to>` (the URL contract from #5). `jj_intake::change_for_jj` and `jj_intake::observation` are the published lookup API for #5.

### Decision: Configuration

**Choice:** `intake.jujutsu` (optional): `workspace_path` and `executable` (absolute, both or neither), `change_id_header` (bool) and `max_stack` (1..=32). Absent means disabled and serializes nothing, keeping the profile binding unchanged. Typed in Nickel with positive and negative fixtures.

## Risks / Trade-offs

- jj may populate its derived index cache when loading an operation; Kiln writes no operation, ref, bookmark or working copy. The read-only test compares op heads, op store, refs and the working copy byte-for-byte.
- The jj CLI template language can change between jj releases; templates are fixed strings and parsing fails closed (`review_jj_observation_invalid`).
- Multi-member intake is not atomic across members: each member is journaled; a provider failure mid-stack leaves earlier members registered and a retry resumes. All admission runs before the first effect.
- The service account needs read access to the jj workspace; the operator owns that boundary.
- Stack intake owns the dependency edges of its members and replaces manually set edges on those changes.

## Verification Strategy

- `kiln-core` unit tests: change-id validator (valid, prefix, uppercase, root), `conflicted_commit` admission, serde omission.
- `kiln-adapter-jj` tests: pure parser/revset tests plus real `jj` fixture repositories (scratch dirs) for resolution, divergence, conflicted bookmark, conflicted commit, missing change and a byte-identical read-only check.
- `kiln-web-service` tests driving `Workspace::command` with a real jj fixture repo and the local Git review provider: rewritten change (3 revisions, predecessors), refusals (no journal or state change), duplicates, 4-change stack, partial edit (exactly 2 appended), reorder, removal flag, header path, legacy-state byte-identical replay and the internal action not being deserializable.
- Nickel typecheck/export with positive and negative fixtures; `node --test tests/live-console-ui.test.mjs`; disposable-instance smoke on 127.0.0.1:9562.
