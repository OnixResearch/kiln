## Context

Kiln's forge adapters are `kiln-adapter-github` and `kiln-adapter-radicle`:

- `kiln-adapter-github` has signed intake, durable replay, a core `StartRun` plan per event, `WorkflowPort` dispatch, and status publication with `Unknown` on uncertainty.
- `kiln-adapter-radicle` is the broker-compatible adapter.

`docs/provider-authoring.md` requires narrow ports, exact revisions, `Unknown` on transport uncertainty, and tests for both valid and invalid input. Tangled is documented in `docs/tangled-integration.md`, built from the Tangled docs and core source at `203809e6`:

- Knots post `org.tangled.event.notifyEvent` with a service-auth JWT signed by the knot's `did:web` key (ES256K).
- Spindles write no records. Status is read from spindle XRPC queries.
- Repositories are named by a knot-minted `did:plc`.
- Object ids are SHA-256 by default.

Peers in this wave added `kiln_core::untrusted` (#12) and `kiln_core::trigger_filter` (#13). This change uses both directly.

## Goals and Success Contract

A fixture knot push, signed like a real knot token, reaches Kiln's spindle XRPC endpoint and produces:

- one exact-revision Kiln run for each reviewed workflow that matches;
- a pipeline id returned to the knot;
- the pipeline view published to a local status endpoint and served by `getPipeline`.

Replaying the same push, or the same token, creates no second run. The following are rejected before any fetch or run:

- a bad signature, wrong audience or method, expired or over-long token, or reused `jti`;
- an unknown repository DID, or a knot that does not host the repository;
- a malformed XRPC body or parameter;
- a workflow with an unsupported key or `when` key.

Untrusted pull requests are blocked until an approver approves them, and then run under the untrusted policy.

False completion includes:
- acceptance without verifying the signature over the exact token bytes;
- a run bound to a ref instead of a verified object id;
- success reported when status delivery or dispatch is uncertain;
- a Tangled workflow field silently ignored;
- untrusted code dispatched without approval;
- Tangled DIDs used as Kiln primary identities.

## Decisions

### Decision: Kiln acts as the spindle

**Choice:** Kiln serves the spindle XRPC methods itself. It does not sit behind a Tangled spindle. A knot selects Kiln through the repository manifest. Kiln advertises `#tangled_spindle` in its DID document; serving that document is operator deployment work.

**Rationale:**
- Exact-commit gating needs Kiln to own acquisition and run identity.
- Webhook payloads from a Tangled spindle do not name the repository DID, so they cannot bind identity safely.

### Decision: Pinned repository identity and service-auth verification

**Choice:** A Nickel profile pins every served repository DID to these values:

- its knot DID;
- its HTTPS clone URL;
- its trusted actors (with push access);
- its approvers.

Verification works as follows:

- The pure core verifies ES256K and ES256 compact JWS signatures over the exact token bytes, with low-S required. It checks `aud`, `lxm`, `exp`, `iat`, a bounded lifetime and `jti`.
- Keys come from parsed DID documents (`#atproto` Multikey, `publicKeyMultibase`). The shell resolves `did:web` from `/.well-known/did.json` and `did:plc` from the configured PLC directory.
- If key resolution is uncertain, Kiln returns 503, so the knot retries.

**Rationale:** Following the repository DID document's `#tangled_knot` needs a live PLC resolution trust decision. Pinning keeps admission reviewed and fail-closed.

### Decision: Durable replay

**Choice:** A SQLite store keeps:

- pipelines under a unique replay key (BLAKE3 over a canonical candidate encoding);
- used token `jti`s until they expire.

Replay keys work as follows:

- **Push:** `(repo, ref, oldSha, newSha)`, as in Tangled.
- **Manual and pull request:** the key includes the verified `jti`.
- **Retry:** derived from the source pipeline and a retry ordinal.

**Rationale:** This matches spindle replay semantics, and the knot's 3-day retries cannot duplicate runs.

### Decision: Reviewed workflow subset

**Choice:** A pure `workflows` module parses YAML into a typed reviewed workflow with path-qualified diagnostics.

Accepted:
- `when` with `push`, `pull_request` and `manual`, and `branch`, `tag`, `types` and `paths`;
- `engine` and `image` that the profile declares;
- `dependencies` that are a subset of the engine's declared packages;
- `clone` (`skip`, `depth`, `tags`; `submodules` only when false);
- `environment`;
- `steps` (`name`, `command`, `environment`).

Refused: everything else, including `schedule`, `cache`, `runs_on`, `audience`, `registry`, `substituters`, `services` and `virtualisation`.

`when` maps onto `kiln_core::trigger_filter` (#13) filters (`branches`, `tags`, `types` and `paths`), admitted with `admit_filter` and evaluated with `evaluate`. Changed-file facts use `kiln_core::monorepo::ChangedFiles`; unknown changed files run the workflow.

**Rationale:** Tangled workflows are executable configuration. Silently dropping a field would change their meaning.

### Decision: Pipeline record and Kiln runs

**Choice:** A pure planner turns an admitted event and an exact acquisition into a `PipelineRecord`:

- The pipeline id is `kiln-` followed by 32 hex digits of the pipeline identity.
- It holds one Kiln run per matched workflow.
- Each run is a `WorkflowEffectPlan` with `WorkflowSourceContext{provider: tangled, repository: repo DID, revision: object id}`. Its single node's `operation_identity` is the BLAKE3 identity of the reviewed workflow at that commit.
- `kiln_core::domain` `StartRun`, `CompleteAttempt`, `RecordUnknown` and `ReconcileAttempt` events are applied as in the GitHub adapter.

Kiln outcomes map to Tangled statuses:

| Kiln outcome | Tangled status |
| --- | --- |
| Pending | `pending` |
| Held | `blocked` |
| Active | `running` |
| Success | `success` |
| DomainFailure, Denied, ProviderError | `failed` |
| TimedOut | `timeout` |
| Cancelled | `cancelled` |
| Unknown | `pending`, with an `error` saying the outcome is unknown; never `success` |

**Rationale:** This uses the existing port and core contracts, so provider identities stay provider facts.

### Decision: Untrusted pull requests follow #12

**Choice:**
- Pull request triggers arrive through `org.tangled.ci.triggerPipeline` with an authenticated caller.
- Trust uses the #12 classification facts:
  - `same_repository` is true when `sourceRepo` is absent or equals the repository DID;
  - the author association is `COLLABORATOR` when the author is a pinned trusted actor.
- An untrusted pipeline is `blocked`. `approvePipeline` by a pinned approver approves it for the pipeline's exact commit. Any other caller is refused, and `PipelineNotBlocked` is returned when nothing is blocked.
- Approved pipelines and their retries dispatch with the profile's `untrusted_policy_id`.
- The adapter calls `kiln_core::untrusted::classify` and `decide_approval` directly, and stores an `UntrustedGate` on the pipeline. A pinned approver acts as the `Operator` authority. Tangled pull requests have no numbers, so the gate number is derived from the BLAKE3 hash of the pull AT-URI.

**Rationale:** This is Tangled's rule ("approving lets the code run, untrusted"), and #12 is its Kiln counterpart.

### Decision: Status publication

**Choice:**
- `getPipeline` serves the `org.tangled.ci.defs#pipeline` view.
- A `StatusPort` posts the same view as JSON with `X-Tangled-Event: org.tangled.ci.pipeline` and `X-Tangled-Signature-256` (HMAC-SHA256) to the profile endpoint.
- Delivery is reserved before sending. A lost response becomes `Unknown`, and the next tick re-delivers the same request identity, which is BLAKE3 of the view bytes.

**Rationale:** Tangled has no push-based status channel. A signed projection gives local consumers verifiable status without claiming a Tangled protocol.

## Risks / Trade-offs

- Approver and trusted-actor sets are pinned rather than read from collaborator records. They can drift from Tangled's collaborator list.
- Pull request intake through Jetstream is absent, so pull request runs need an explicit `triggerPipeline` caller.
- Pipeline ids are not TIDs. The knot accepts them; app view acceptance is unverified.
- The protocol is unversioned and changed recently. Kiln pins the shapes of 2026-10-30.

## Verification Strategy

- **Pure unit tests:**
  - service-auth (valid ES256K and ES256 tokens; bad signature, high-S, wrong aud, lxm or iss, expired, over-long, missing jti, unsupported alg);
  - DID document key parsing;
  - status-delivery HMAC signing and verification (valid and invalid);
  - notifyEvent admission (malformed JSON, unknown repo DID, repo mismatch, non-hex sha, deletions, skip-ci, refs outside heads and tags);
  - workflow mapping (accepted subset, unsupported keys, unsupported `when` keys, schedule);
  - `when` evaluation;
  - pipeline planning and status mapping;
  - approval transitions.
- **Shell tests:**
  - the SQLite replay store (replay returns the same pipeline, jti reuse refused, durable across reopen);
  - Git CLI exact acquisition of SHA-1 and SHA-256 fixture repositories (good commit, unknown commit, mismatched object format);
  - the status publisher against a local endpoint (HMAC verified; a dropped connection yields `Unknown`).
- **End-to-end test:** over loopback HTTP, a fixture knot posts a signed push. Kiln:
  - fetches the exact commit from a local repository;
  - dispatches through a fixture `WorkflowPort`;
  - publishes status to a local endpoint;
  - serves `getPipeline`.

  The test also covers replay, malformed XRPC, a bad signature and an unknown repo DID, and an untrusted pull request blocked until it is approved.
- **Nickel:** typecheck and export of the positive and negative fixtures.
