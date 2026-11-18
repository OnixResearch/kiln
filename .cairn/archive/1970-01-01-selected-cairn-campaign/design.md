# Context

Campaign coordination spans several existing owners with distinct evidence semantics. A selected set cannot be reconstructed from a bounded readiness enumeration: the requested selectors and every resulting obligation must be retained independently, including absent, truncated, blocked, planning-only, completed, and lifecycle-only members. Work claims, fences, attempt budget, candidate verification, publication/integration and native Cairn lifecycle receipts remain with their existing canonical owners.

## Decision

The pure core stores stable campaign identity and immutable selection revision, exact repository/change/task selectors, explicit prerequisite inclusion, per-member selected profile and policy bindings, repository instructions/check/review/integration policy identities, endpoint goal, agent/concurrency/budget lineage, optional decision context, observations, human-decision demand, and pending effect identity. It references canonical work key/version and attempt/fence rather than duplicating their ledgers. A changed contract is a new explicit revision; an active attempt retains its original context and policy.

The controller is a shell over the existing durable implementer boundary. SQLite persists campaign state and a decision/effect intent transactionally before any owner call. Restart prioritizes exact reconciliation of all pending/uncertain effects and never blindly redispatches. Cancellation closes successor admission and fences accepted output; it does not claim provider cancellation or resource release absent owner observations. Overlapping campaign members attach to the canonical work and claim; cancellation only removes that campaign's attachment and never erases shared owner state.

Readiness and graph observations are source/policy/subject/profile-bound, bounded, and incomplete enumeration remains explicit. Artifact authoring/repair is artifact-scoped; coding repair, successor, infra retry, unknown recovery and semantic replan are separate decisions. A pure evidence-aware selector may progress independent work while prerequisites wait. Profile-specific workflow obligations and required reviews/checks remain intact; native lifecycle completion needs exact Cairn observations and cannot be inferred from checked artifacts or empty readiness.

Optional bounded decision context binds stable decision/ADR identity, revision, alternative, evidence/supersession and projection identity to selection and work version. Per-attempt context is retained exactly. Decision freshness and impact are separate from candidate, work/spec, policy, verifier and integration-target freshness. Preference demand, empirical uncertainty and required review are separate durable requests; configured outstanding bounds retain unissued obligations and impose backpressure without waiving gates. Missing optional context is visible/nonblocking; summaries and display tags never grant authority or establish evidence.

## Operator projection

The campaign projection is an explicit versioned read-only leaf over the existing campaign/current-work owners. It retains every selected obligation and source/freshness limitation rather than presenting an empty ready set as completion. Live API, CLI and inert offline HTML share one bounded redacted semantic payload and identity; the final identity is computed only after canonical current-work and optional exact resource joins, with the identity field cleared during hashing. Resource joins enrich admitted bindings without authorizing readiness or masking independent inventory failures. Rendering and proposed next steps never grant controller, successor, landing or lifecycle authority.

## Non-goals

No Mneme runtime, new worker-memory checkpoint, real-worker qualification, credentials, remote publication/integration, Cairn lifecycle mutation, endpoint fallback, or change to parent-owned registrations and global release/changelog files. The pinned Cairn cohort is authoritative; adapters consume existing checked-in producer contracts only.
