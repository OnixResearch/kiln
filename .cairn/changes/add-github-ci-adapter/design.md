## Context

The core already represents exact Git facts and opaque workflow source context. The local Git adapter reads existing objects; it does not fetch or check out. The Lattice adapter supplies bounded Unix dispatch, observation and recovery, while the producer-owned workflow-exchange application executes exact saved workflows and injects three source-context environment keys.

## Decisions

### Decision: Keep provider protocols in a dedicated shell

**Choice:** Add `kiln-adapter-github`, with pure event/profile/status decisions and thin HTTP, SQLite, SecretSpec, Git acquisition and process shells. Reuse the existing core workflow admission and Lattice adapter. Do not change core/provider interfaces or couple the GitHub adapter to Radicle implementation crates.

**Rationale:** GitHub is a forge and trigger/status provider, not a new VCS or a replacement CI identity authority.

### Decision: Authenticate before intake and acknowledge durable admission

**Choice:** Verify `X-Hub-Signature-256` as HMAC-SHA256 over bounded original body bytes before JSON decoding or durable/external effects. Require bounded, unambiguous delivery/event headers and an admitted repository. Support pushes and PR opened/reopened/synchronize events; ignore deletion, closed PRs and non-executable events. Fork execution is an explicit denied-by-default policy. Persist one exact request before acknowledging, then execute outside the HTTP acknowledgement path.

**Rationale:** GitHub requires prompt acknowledgements. Delivery IDs are not signed: bind them to body identity and also deduplicate exact candidate/pipeline identities. A changed body cannot reuse a delivery ID.

### Decision: Reserve once and recover instead of redispatching

**Choice:** Use bounded SQLite records with immediate transactions and full synchronization. Separate queued, dispatch-reserved, accepted/unknown and terminal states. Only a newly reserved dispatch may call `dispatch`; an interrupted reservation must call `recover_dispatch`. Admit every observation against the stored plan before changing CI state. Bound worker concurrency, polling and retained records. Database/profile disagreement fails closed.

**Rationale:** A process crash or missing response does not establish that an external effect did not start. Exhausted recovery or observation remains unknown and never creates permission to execute again.

### Decision: Use an exact-source command inside a reviewed Lattice workflow

**Choice:** Add `kiln-github-job`, consuming only the fixed Lattice source provider/repository/revision keys and a BLAKE3-bound typed job profile. Acquire the configured GitHub repository's exact commit using a bounded child process, validate it through the existing Git snapshot adapter, materialize a fresh owned checkout, then run the fixed reviewed argv with bounded output, deadline and process-group teardown. Source credentials are not forwarded to the CI command. The workflow operation and saved command must pin the job profile identity.

**Rationale:** Source context alone does not acquire source. This makes the missing effect explicit without adding a second workflow runtime or treating a webhook branch name as source identity.

### Decision: Publish bounded exact commit statuses

**Choice:** Use GitHub REST API version `2022-11-28`, commit-status endpoints and a fixed admitted status context. Bind the description to the Kiln run identity and the request to repository, commit, state and pipeline. Resolve bearer and webhook secrets through pinned SecretSpec execution; never put credentials in job environment, URLs, state, diagnostics or logs. Disable HTTP redirects and automatic mutation retries. Persist a write reservation before POST; reconcile ambiguous writes by read-only exact status lookup instead of blindly posting again. A failed publication cannot change the CI outcome.

**Rationale:** A successful HTTP exchange is not an admitted execution result; a lost POST response is not evidence that a status was not created.

## Risks / Trade-offs

- A local HTTP listener is intended behind an operator-managed HTTPS reverse proxy. HMAC authenticates payload bytes, not delivery headers or global freshness.
- Retention and connection limits are explicit and fail closed; no silent eviction may erase replay protection. Operators own rotation and recovery.
- Bounded Exec and a temporary checkout are not a sandbox. Run jobs under a separate worker identity and appropriate host isolation; enabling untrusted forks requires an explicit operator policy.
- GitHub credentials, installed Lattice runtime/routes, repository availability and live status-write authority are deployment prerequisites, not invented fixtures or fallback implementations.
- Local protocol/command evidence does not establish live GitHub availability, source trust, workflow correctness, confinement, deployment success or release eligibility.
