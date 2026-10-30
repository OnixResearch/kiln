# Tangled integration

This note records how Tangled delivers CI events, which parts map onto Kiln
ports, and what has not been established. It is the investigation deliverable
of issue #14. The `kiln-adapter-tangled` crate implements the parts this note
supports.

Tangled is a forge built on AT Protocol. Repositories live on *knots*. CI runs
on *spindles*. Kiln acts as a spindle-compatible service: a knot notifies Kiln,
and Kiln runs the matching workflows at the exact pushed commit. Kiln does not
add federation or AT Protocol identity to its core. The adapter is an edge, like
the GitHub and Radicle adapters.

## Sources

All sources were read on 2026-10-30. Tangled changes quickly: two of the
lexicons below changed in the two days before this reading.

| Source | What it gave |
| --- | --- |
| [Spindle XRPC API](https://tangled.org/docs/api/spindle/) | Method list and errors for `org.tangled.ci.*`, `org.tangled.event.notifyEvent`, secrets and webhooks |
| [How CI works](https://tangled.org/docs/spindles/) | The push → `notifyEvent` → pipeline model; spindles write no records |
| [Writing workflows](https://tangled.org/docs/spindles/writing-workflows/) and [Workflow reference](https://tangled.org/docs/spindles/workflows/) | Workflow fields, `when` semantics, clone defaults, environment, pull request approval |
| [Secrets](https://tangled.org/docs/spindles/secrets/) | Untrusted runs get no secrets, caches or ID tokens |
| [Run a spindle](https://tangled.org/docs/self-hosting/spindle/) and [Self-hosting](https://tangled.org/docs/spindles/self-hosting/) | Spindle signing key, admission, DID document |
| [Identifiers and Git URLs](https://tangled.org/docs/reference/identifiers/) | Repository DIDs, service `did:web`, clone URLs, SHA-256 object format, push options |
| [Records](https://tangled.org/docs/reference/records/) | `org.tangled.repo.manifest`, `org.tangled.review.patch`, `org.tangled.git.refs` |
| [Knot XRPC API](https://tangled.org/docs/api/knot/) | `org.tangled.repo.mintToken` and service-auth token limits |
| Source, [tangled.org/tangled.org/core](https://tangled.org/tangled.org/core) at commit `203809e6f0e061febbf5cb56e8ca135e279c63ee` (2026-10-30T22:10:16+02:00) | Lexicons under `lexicons/org/tangled/{event,ci,webhook}`; spindle `notify_event.go`, `xrpc/org_tangled_event_notify_event.go`, `xrpc/xrpc.go`, `xrpc/org_tangled_ci_approve_pipeline.go`, `webhook/webhook.go`, `models/webhook.go`; `xrpc/serviceauth/service_auth.go`; knot `knot2/crates/knot-xrpc/src/outbox.rs`, `knot-atproto/src/{lib,jwt,auth,resolve}.rs`, `knot-runtime/src/signer.rs` |

## Event delivery from knot to spindle

1. A user pushes to the knot. The knot reads the spindle DID from the
   repository's `org.tangled.repo.manifest` record.
2. The knot resolves the spindle's DID document
   (`https://<host>/.well-known/did.json`) and takes the service entry whose id
   ends in `#tangled_spindle` and whose type is `TangledSpindle`.
3. The knot posts `org.tangled.event.notifyEvent` to
   `<endpoint>/xrpc/org.tangled.event.notifyEvent`. The input is
   `{repo: <repo DID>, event: org.tangled.event.push}`. The push object has
   `repo`, `ref`, `oldSha`, `newSha`, `actor` (the pusher's DID) and optional
   `pushOptions`.
4. The knot keeps the event in a durable outbox:
   - It retries with backoff from 1 s up to 60 s.
   - It gives up after 3 days.
   - It drops the event on any 4xx answer other than 408 and 429.
   - It delivers only the oldest entry per spindle at a time.
5. The output is `{ci: {pipelines: [<id>], message}}`. It changed from
   `{pipeline, message}` in commit `49dc5d90`, two days before this reading.
   The published API page still shows the older shape. The knot accepts any non-empty pipeline id without control
   characters, up to 256 bytes.

Spindle behaviour that Kiln must reproduce:

- It ignores refs outside `refs/heads/` and `refs/tags/`.
- It ignores deletions (an all-zero `newSha`).
- It ignores pushes with the `skip-ci` or `ci-skip` push option.
- It rejects object ids that are not hex with 422. A retry cannot fix that.
- A push replay with the same `(repo, ref, oldSha, newSha)` returns the
  existing pipeline and starts nothing new.
- It returns 503 when it cannot find the repository's knot or is busy, so that
  the knot retries.

Manual runs use the same method with an `org.tangled.ci.trigger.manual` event
(`repo`, `commit`, optional `ref`, `workflows` and `inputs`). They take the user
authentication path instead of the knot path.

Pull request runs do not arrive through `notifyEvent`. The spindle watches the
Jetstream for `org.tangled.track.ticket` and `org.tangled.review.patch` records.
`org.tangled.ci.triggerPipeline` also accepts an `org.tangled.event.pullRequest`
trigger.

## Spindle authentication

Authenticated methods carry an AT Protocol service-auth JWT as
`Authorization: Bearer`.

- **Claims:** `iss`, `aud` (the spindle's `did:web`), `exp`, `iat`, `jti`, and
  `lxm` (the method NSID).
- **Algorithms:** knots sign with secp256k1 (`ES256K`, `knot-runtime`
  `K256Signer`). User tokens come from the user's PDS. Spindles hold a P-256
  key.
- **Verification:** the spindle checks `aud`, `lxm` and `exp`. It resolves the
  issuer's DID document and verifies the signature with the `#atproto` key.
- **notifyEvent rules:**
  - The issuer must be a `did:web`, the knot that hosts the repository.
  - The `jti` must not have been seen before.
  - The token must not live longer than 300 s. Knots mint 60 s tokens.
  - The push `repo` must equal the request `repo`.
  - The push `actor` must be present.
- **approvePipeline and retryPipeline:** the caller must be the repository owner
  or a collaborator, and the pipeline must belong to the named repository.

Webhooks are a separate, outgoing channel. A spindle posts `push`,
`repository:renamed` and `pull_request:*` deliveries. Each delivery carries
these headers:

- `X-Tangled-Event`;
- `X-Tangled-Delivery`;
- `X-Tangled-Hook-ID`;
- `X-Tangled-Repo` (`<owner DID>/<rkey>`);
- `X-Tangled-Signature-256: sha256=<hex HMAC-SHA256 of the body>`, present only
  when the webhook has a secret.

Webhook payloads name the owner DID and repository rkey, but not the repository
DID.

## Status publication

A spindle writes no records. Pipelines, statuses and logs live in the spindle's
database:

- The web app reads them with `org.tangled.ci.getPipeline` and
  `org.tangled.ci.queryPipelines`.
- Logs stream over `org.tangled.ci.subscribePipelineLogs`, a WebSocket.
- A pipeline is an `org.tangled.ci.defs#pipeline` view. Each workflow run has a
  `status` from `pending`, `blocked`, `running`, `failed`, `timeout`,
  `cancelled` and `success`.

Status is therefore pulled, not pushed. Kiln publishes status in two ways:

1. It serves `getPipeline` with the `#pipeline` view.
2. It posts the same view to an operator-declared endpoint. The post is signed
   with the Tangled webhook HMAC header format, so a local consumer can verify
   it. Tangled itself defines no status webhook event.

## Repository identity

Every repository has its own `did:plc`, minted by its knot. The DID survives
renames and transfers. Pipelines, secrets and webhooks all key on it. Knots and
spindles use `did:web:<host>`. The repository DID document lists the knot as
`#tangled_knot` (type `TangledKnot`).

Kiln keeps Tangled identities as provider facts:

- `ExternalRef("tangled", "repository", <repo DID>)`;
- `ExternalRef("tangled", "revision", <object id>)`;
- `ExternalRef("tangled", "pipeline", <pipeline id>)`.

Kiln's own run, candidate and pipeline-record identities are BLAKE3. The
operator pins each served repository DID to its knot DID, clone URL, trusted
actors and approvers in a reviewed Nickel profile. An unknown repository DID is
refused before any fetch.

## Exact-commit source acquisition

- The clone URL is `https://<knot host>/<repo DID>`. The DID form survives
  renames, and a trailing `.git` is not accepted.
- The spindle fetches only the triggering commit (`clone.depth` 1) and checks
  it out detached.
- New repositories use SHA-256 (64 hex digits). Repositories migrated from the
  Go knot keep SHA-1 (40 hex digits).
- Kiln fetches the exact object id into a private object store of the matching
  format. It verifies that the object is a commit with exactly that id, and
  reads `.tangled/workflows/*.yml` at that commit.
- Changed files come from a diff against `oldSha` when that commit can be
  fetched. Otherwise the changed-file set is unknown, and path filters run the
  workflow, as in Tangled.

## Workflows

`.tangled/workflows` files are not executed verbatim. A reviewed subset is
mapped, and every other field fails closed with a path-qualified diagnostic,
like the Actions `migrate` subset.

| Tangled field | Kiln mapping |
| --- | --- |
| `when` with `event` from `push`, `pull_request` and `manual`, and `branch`, `tag`, `types` and `paths` | Trigger filter. The glob rules are `*` within a segment and `**` across segments. |
| `engine`, `image` | Must name an engine and image the profile declares. Recorded as provider facts. |
| `dependencies` | Must be a subset of the packages the profile declares for that engine. |
| `clone.depth`, `clone.skip`, `clone.tags` | Recorded. `clone.submodules: true` is refused. |
| `environment` (workflow or step) | Literal string map. |
| `steps[].name`, `steps[].command` | Ordered shell steps. |
| `schedule`, `cache`, `runs_on`, `audience`, `registry`, `substituters`, `services`, `virtualisation`, unknown keys | Refused. |

`schedule` is refused because Tangled hashes `H` fields from the repository DID
and file name, so Kiln's own schedules would fire at different minutes.

Pull request runs follow the untrusted-run rules of #12:

- **Trusted:** a branch of the same repository whose author is a pinned trusted
  actor. The run starts.
- **Untrusted:** any other pull request. All of its workflows are `blocked`
  until a pinned approver calls `approvePipeline`.
- An approved run, and every retry of it, uses the profile's untrusted workflow
  policy, which grants no secrets.

## Mapping onto Kiln ports

| Tangled concept | Kiln port or type |
| --- | --- |
| `notifyEvent` push or manual event | `events::admit_notify` (pure), then `pipeline::plan_pipeline`, which applies `kiln_core::domain` `StartRun` per matched workflow |
| Pipeline (one event) | `PipelineRecord`, holding one Kiln run per workflow. The pipeline id is `kiln-` followed by 32 hex digits of its BLAKE3 identity. |
| Workflow run execution | `kiln_provider_api::WorkflowPort`, with `WorkflowSourceContext { provider: tangled, repository: <repo DID>, revision: <object id> }` and the node's `operation_identity` set to the reviewed definition's BLAKE3 identity |
| Exact commit fetch | `source::SourcePort`, implemented by `GitCliSource` (bounded Git CLI; SHA-1 and SHA-256) |
| Status | `getPipeline` and `status::StatusPort`. `HttpStatusPublisher` signs with `X-Tangled-Signature-256` and sends `X-Tangled-Event: org.tangled.ci.pipeline`. An uncertain delivery is `Unknown` and is re-sent. |
| `approvePipeline` | `kiln_core::untrusted::decide_approval`. A pinned approver acts as the `Operator` authority; anyone else as `Viewer`. The approval is bound to the pipeline's exact commit. |
| Untrusted classification | `kiln_core::untrusted::classify(Approve, …)`. `same_repository` means a source branch with no foreign `sourceRepo`. A caller who is a pinned trusted actor counts as push access. |
| `when` | `kiln_core::trigger_filter::{admit_filter, evaluate}` |
| `retryPipeline` | `pipeline::plan_retry`: a new pipeline bound to the same commit and stored reviewed definitions. An approved gate is copied, so the retry runs untrusted. |
| `listWorkflowDefinitions`, `getWorkflowDefinition` | The reviewed mapping read at an exact resolved commit |

Kiln outcomes map to Tangled workflow statuses as follows:

| Kiln | Tangled status |
| --- | --- |
| Held by #12 | `blocked` |
| Queued or reserved | `pending` |
| Active | `running` |
| Success | `success` |
| DomainFailure, Denied, ProviderError | `failed` |
| TimedOut | `timeout` |
| Cancelled | `cancelled` |
| Unknown or unresolved | `pending`, with `error` set to "outcome unknown". Never `success`. |

Tangled pull requests have record keys, not numbers. The #12 gate's
`pull_request` number is therefore the first eight bytes of the BLAKE3 hash of
the pull request AT-URI.

The profile is `config/tangled-spindle.ncl`, with contracts in
`config/tangled-contracts.ncl` and fixtures in `config/fixtures/*tangled*.ncl`.
The exported positive fixture is `fixtures/tangled/spindle-profile.json`. The
default test suite runs without Git. The `git-fixtures` feature adds exact
acquisition from local SHA-1 and SHA-256 repositories.

## Read versus needs verification

**Read in source or documentation:**

- the notifyEvent input and output, the push and manual objects, and the knot
  retry and drop rules;
- the service-auth claims and the issuer and replay rules for notifyEvent;
- the approve, retry, list and get methods and their errors;
- the webhook signature header;
- the `#pipeline` and `#workflow` views and the workflow status values;
- the workflow fields, `when` semantics and clone defaults;
- the untrusted pull request rules.

**Not established (needs verification against a live knot and spindle):**

- **Live interoperability.** No real knot has called Kiln. All evidence is
  fixture-based, built from the lexicons and source above.
- **DID resolution.** Kiln resolves `did:web` and `did:plc` documents through
  configured HTTPS endpoints. It has not been checked against
  `plc.directory`-hosted repository DIDs, handle resolution or key rotation.
  Kiln does not follow `#tangled_knot` from the repository DID document. The
  operator pins the knot instead.
- **Pull request intake.** Kiln does not consume the Jetstream. Pull request
  events reach Kiln only through `org.tangled.ci.triggerPipeline` with an
  `org.tangled.event.pullRequest` trigger, from an authenticated caller.
  Patch-based pull requests, which have no source branch, are not supported.
- **Out-of-scope methods.** Kiln does not serve secrets (`org.tangled.secret.*`),
  webhooks management (`org.tangled.webhook.*`), `subscribePipelineLogs`,
  `cancelPipeline`, `queryPipelines`, `describeSpindle` or quotas. The Tangled
  web app may expect some of these from a selected spindle.
- **Schedules.** Schedule triggers are not supported (see Workflows).
- **App view acceptance.** It is unknown whether the Tangled app view accepts a
  third-party spindle whose pipeline ids are not TIDs. The knot accepts them.
- **Protocol stability.** The notifyEvent output changed two days before this
  reading, and the protocol is not versioned. Kiln pins the shapes listed above.
- **Running behind a spindle.** Kiln does not take Tangled webhook deliveries
  as input, because their payloads do not name the repository DID. Kiln only
  produces the webhook signature format for its status projection.
- **Pull request rounds.** A newer round of the same pull request does not yet
  supersede an older blocked pipeline (`kiln_core::untrusted::supersedes`).
- **Deployment.** There is no deployment binary, Lattice `WorkflowPort` wiring,
  DID document for the Kiln spindle, or systemd unit yet. The library shell is
  `server::serve` with a caller-supplied `WorkflowPort`.
