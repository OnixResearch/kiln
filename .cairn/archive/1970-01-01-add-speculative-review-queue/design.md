## Context

`crates/kiln-web-service/src/code_review/queue.rs` keeps `State.queue: Vec<Entry>`. On each tick, `advance_queue_inner` selects the first active entry of each target ref and runs `advance_entry` on it, which does three things:

1. builds one exact merge candidate through `ReviewOperation::BuildIntegration`, journalled by `reserve_apply`;
2. starts canonical CI for that candidate, once per affected monorepo project at its scope identity;
3. once CI passes, runs `PrepareSubmit` and `Submit` through the ordinary command path. That path rechecks approvals, owners, policy, dependencies and full/clean CI against the integration fact from `queue::integration`, and submits with exact target CAS.

`kiln-core` `gating.rs` has `GateMode`, `WindowPolicy`, `admitted_candidate_indices` and `update_window`, and the review queue does not use any of them. The local Git provider builds integrations only directly on the exact current target. The engine dispatches at most one active run per project; further runs wait in the `queued` phase.

## Goals and Success Contract

With `mode: speculative` and a window of N, the queue builds a stack of up to N exact candidates per target, each on its assumed predecessors. It admits canonical CI for every candidate before the head submits, and submits strictly in queue order. Each submission passes fresh admission and exact target CAS.

A failing entry is ejected alone. The later entries are rebuilt with fresh CI, and old results never qualify a new chain. Restarting at any point resumes from durable state without a duplicate CI dispatch or provider operation. With `mode` absent or `serial`, the behaviour, the identities and the serialized bytes of profiles, queue entries and journals are unchanged.

False completion includes any of the following:
- submitting an entry before its predecessors land;
- submitting a candidate whose chain is not the target's actual history;
- accepting CI from a superseded candidate;
- ejecting entries other than the failing one;
- starting CI for a candidate twice after restart;
- serial records gaining keys;
- advisory approvals counting toward a submit.

## Decisions

### Decision: Profile shape and defaults

**Choice:** `queue::Profile` gains `#[serde(default, skip_serializing_if = "is_serial")] mode: QueueMode` and `#[serde(default, skip_serializing_if = "Option::is_none")] window: Option<WindowPolicy>`. `QueueMode` is `serial` or `speculative`, and maps to `kiln_core::gating::GateMode`. Admission rules:
- `speculative` requires `window` with `1 <= floor <= ceiling <= MAX_SPECULATIVE_WINDOW` (16), `ceiling <= max_entries`, and `increase` and `decrease` each between 1 and `ceiling`.
- `speculative` requires the `speculative_integration` capability.
- `serial` rejects any `window`.

The Nickel `Queue` contract adds `mode | optional` and `window | optional`, with the same bounds plus a cross-field predicate. Export omits absent fields, so existing inputs export byte-identical JSON.

**Rationale:** the default and the omission keep every existing profile replaying unchanged. Admission requires the capability so that a provider which cannot build speculative candidates fails before any external effect.

### Decision: Speculative provider operation

**Choice:** add a new variant, `ReviewOperation::BuildSpeculativeIntegration { commit, expected_target, predecessor, strategy }`, with `ReviewCapability::SpeculativeIntegration`. `BuildIntegration` is left as it is. The Git adapter does the following:
- **Validation:** `predecessor` must be a strict descendant of `expected_target`.
- **Build:** it builds the merge (or rebase) of `commit` onto `predecessor`, using exactly the same code path as `BuildIntegration` with `predecessor` as the parent.
- **Target CAS:** `target == expected_target` is checked before and after construction, as for `BuildIntegration`.
- **Request identity:** the request blob uses kind `build_speculative_integration` and appends the predecessor as a length-prefixed field. It never changes any existing operation's request identity.
- **Recorded facts:** the merge message records `Kiln-Predecessor`.

`REVIEW_CAPABILITY_COUNT` and the Nickel capability list grow by one.

**Rationale:** a new variant means existing operation identities and journals can never change bytes. An explicit predecessor that must descend from the exact target is what allows the candidate to fast-forward the target once its predecessors land.

### Decision: Durable assumed predecessor chain

**Choice:** `Entry` gains `#[serde(default, skip_serializing_if = "Option::is_none")] speculation: Option<Speculation>`. `Speculation` holds:
- `chain`: the BLAKE3 chain identity;
- `root_target`: the exact target observed when the chain was built;
- `predecessors: Vec<Link { queue_id, generation, candidate_commit }>`;
- `superseded: Vec<String>`: retired chain identities, bounded by `MAX_SUPERSEDED_CHAINS` (32).

For a speculative entry, `target_commit` is the commit it fast-forwards at submit, which is the last predecessor candidate or the root. This lets the unchanged `queue::integration` fact and the domain Submit admit it only once its predecessors have landed. `State` gains `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")] queue_windows: BTreeMap<target_ref, usize>`. A missing entry starts the window at `ceiling`.

`Completion::Integration` gains `#[serde(default, skip_serializing_if = "Option::is_none")] chain: Option<String>`. A late completion whose chain or generation no longer matches the entry is recorded as an applied provider operation, but it never sets the candidate.

**Rationale:** the chain lives in the same full-sync SQLite state commit that already persists the queue, so the journal and the queue cannot diverge.

### Decision: Pure planner and thin shell

**Choice:** a new module, `code_review/speculation.rs`, holds the pure functions. It has no I/O.

| Function | Decides |
| --- | --- |
| `chain_identity` | The BLAKE3 identity of a chain |
| `chain_valid(speculation, observed_target, ahead)` | Whether the observed target lies on the chain, and whether the chain's predecessors that have not landed are exactly the active entries ahead, with their current generation and candidate |
| `ci_verdict(outcomes)` | `Pending`, `Passed` or `Failed` |
| `next_window(window, passed, policy)` | The new window, through `update_window` |
| `admitted(window, active)` | The admitted positions, through `admitted_candidate_indices` |

The shell entry point is `advance_speculative(target)` in `queue.rs`. It walks the admitted positions in queue order:
- **Invalid chain:** supersede the entry. Cancel its queued CI runs through the new `Engine::cancel_review_candidate`, which uses the existing queued-cancel path, and reset its candidate, runs and chain.
- **No candidate:** build it on the current target and the candidates ahead, once every entry ahead has a candidate. Entry 0 uses the plain `BuildIntegration`.
- **Candidate but no runs:** start CI with deterministic request identities derived from entry, generation, chain and project.
- **Passed head whose predecessors all landed:** run the existing prepare and submit path.
- **Failed head:** eject it with the serial status and detail, shrink the window, and continue so the remainder rebuilds.
- **Failed non-head, or an unresolved provider operation:** stop extending the window. The failure becomes conclusive only once the entry is head; a predecessor ejection supersedes it anyway.

Serial mode keeps `advance_entry` byte for byte. Only the CI-start and submit blocks are extracted into helpers shared by both modes, and those helpers keep identical identities.

**Rationale:** this follows the functional core and imperative shell rule. The planner is tested without Git, SQLite or the engine.

### Decision: Authority never transfers

**Choice:** speculative build admission uses the same `domain::checklist` gate as serial. Every non-CI item must be satisfied, and dependencies must already have landed. Submit uses the unchanged `PrepareSubmit` and `Submit` commands, so current approvals, owners, policy, dependencies and full/clean CI are rechecked for the exact candidate. Advisory approvals (#8) are stored apart from votes and never reach `checklist`, so they never count.

**Rationale:** speculation changes only what is built and tested, never what authorizes a submit.

## Risks / Trade-offs

- **Single-project parallelism:** the engine serializes dispatch per project, so a single-project review admits N runs that execute one after another. Real parallelism needs the existing capacity, such as affected monorepo projects or separate projects. The window still removes the build-to-submit round trips between entries.
- **Wasted CI on failure:** a CI run dispatched for a superseded candidate is ignored rather than cancelled at the provider. Provider cancellation is ambiguous and is out of scope. The run occupies its project slot until it terminates.
- **Late conclusive failures:** a non-head failure is conclusive only at the head. A failing middle entry blocks rebuilding behind it until its predecessors land. This trades latency for never ejecting an entry whose failure was caused by an ancestor.
- **Starting at the ceiling:** the window starts optimistic. A burst of failures shrinks it toward `floor`.

## Verification Strategy

- **Pure planner unit tests** (positive and negative): chain validity under landing, ejection, a foreign target move and a generation change; the CI verdict; window growth and shrinkage at its bounds; admitted positions.
- **Queue integration tests** in `tests/code_review_queue.rs`, using real disposable Git, SQLite and engine:
  1. window 3: three runs are admitted before the first submit, and submissions land in queue order;
  2. a failing middle entry ejects only itself, and the later entry rebuilds, passes fresh CI and lands, while a late success of its superseded run is ignored;
  3. a restart mid-window does not dispatch again or create a new provider operation;
  4. serial byte identity: the queue entry and journal JSON of a serial run contain exactly today's keys, and fixed serial profile, entry, pending and state JSON round-trip byte-identically;
  5. unsupported capability and invalid window profiles are rejected;
  6. an advisory approval alone never qualifies a queued entry.
- **Adapter conformance** for the speculative build:
  - the build lands on the predecessor;
  - a predecessor that does not descend from the target is rejected;
  - a moved target is rejected;
  - a reused identity with another predecessor is rejected;
  - an unsupported capability is rejected.
- **Nickel:** typecheck plus positive and negative fixtures for the mode, the window and the capability.
