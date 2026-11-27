# Single Tenant Web Service Specification

## Purpose

Defines the `single-tenant-web-service` capability.

## Requirements

### Requirement: Typed allowlisted repository enrollment

r[kiln.web.enrollment] The service MUST admit a closed typed single-tenant profile and permit enrollment only of operator-installed repository and workflow profiles. A run MUST bind one admitted exact Git revision, canonical tree, pipeline and provider plan before dispatch. Client-supplied paths, commands and provider substitutions MUST be rejected.

#### Scenario: Enrolled repository starts a run

r[kiln.web.enrollment.scenario.source]
- GIVEN an installed project and an authorized enrollment
- WHEN a run is requested
- THEN its persisted plan MUST bind the actual exact admitted source and reviewed workflow.

#### Scenario: Unknown project or malformed profile

r[kiln.web.enrollment.scenario.denied]
- GIVEN an unknown project, drifted profile or unsupported request
- WHEN admission runs
- THEN no provider effect MAY occur.

### Requirement: Durable canonical lifecycle

r[kiln.web.lifecycle] The shell MUST reuse canonical core transitions and existing Lattice observation admission, durably reserve requests before effects, bind stored state to its service/profile identity, and recover original accepted or ambiguous dispatch without blind redispatch. Replayed matching request IDs MUST return the original resource; conflicting reuse MUST fail. Unknown MUST NOT become success or safe retry.

#### Scenario: Process restarts after dispatch intent

r[kiln.web.lifecycle.scenario.restart]
- GIVEN a durable intent with interrupted dispatch
- WHEN the shell restarts
- THEN it MUST reconcile the original native operation without a second normal dispatch.

#### Scenario: Request identity is reused

r[kiln.web.lifecycle.scenario.replay]
- GIVEN an existing reserved request
- WHEN its request ID is reused
- THEN only byte-equivalent admitted intent MAY return its original resource.

### Requirement: HTTPS role and origin admission

r[kiln.web.authentication] All private API operations MUST authenticate before state/provider access using explicitly separate operator and worker credentials. Registered Host/Origin, bounded framing/connections/deadlines and private credential-file admission MUST fail closed. Worker authority MUST be limited to exact source-bound active-run lookup and bounded captured-output/artifact upload; it MUST NOT enroll, trigger or control. Tokens MUST NOT enter URLs, browser persistent storage, diagnostics, serialized profiles or repository-command environments.

#### Scenario: Operator uses the live interface

r[kiln.web.authentication.scenario.operator]
- GIVEN valid HTTPS origin and operator credential
- WHEN an admitted API operation is requested
- THEN only its explicitly authorized service operation MAY proceed.

#### Scenario: Origin, role or credential is invalid

r[kiln.web.authentication.scenario.denied]
- GIVEN an invalid origin/credential or a worker requesting control
- WHEN API admission runs
- THEN it MUST reject before state mutation or provider effects.

### Requirement: Bounded exact-source worker and logs

r[kiln.web.worker] The fixed-profile worker MUST verify original profile identity, admit the exact Lattice source context and Git snapshot, materialize an owned exact checkout, execute only reviewed commands using existing bounded-execution mechanics and preserve real exit/cancellation outcomes. The service MUST preserve actual provider diagnostic ranges with native cursor, truncation and gap facts. Captured stdout/stderr MUST use separately identified worker-owned byte cursors, exact active source/attempt/execution/node binding, bounded BLAKE3-checked atomic retention and conflicting-replay rejection; it MUST NOT claim to be native provider output.

#### Scenario: Source-bound command emits output

r[kiln.web.worker.scenario.logs]
- GIVEN an admitted exact source and real native workflow
- WHEN its reviewed command executes
- THEN the observed node/log/outcome MUST correspond to that execution, not a sample journal.

#### Scenario: Source or child bounds fail

r[kiln.web.worker.scenario.denied]
- GIVEN source/profile drift, unsafe paths, timeout, cancellation or exceeded output bounds
- WHEN job admission/execution runs
- THEN no fallback source or invented success MAY be reported.

### Requirement: Real identity-bound artifact transport

r[kiln.web.artifacts] Artifact ingress MUST require worker authority and matching active source, attempt, execution and known node. Names, bytes, counts and retained content MUST be bounded and BLAKE3 checked. Atomic owned storage and opaque download IDs MUST prevent caller path traversal, symlink substitution and stale cross-run attachment. Receipts/logs MUST NOT be relabeled as fabricated build products.

#### Scenario: Worker publishes a real output

r[kiln.web.artifacts.scenario.download]
- GIVEN a declared regular-file output from the real worker
- WHEN ingress and authorized download succeed
- THEN downloaded bytes MUST reproduce the admitted artifact identity and exact attempt linkage.

#### Scenario: Artifact binding or content is wrong

r[kiln.web.artifacts.scenario.denied]
- GIVEN stale execution/attempt/source, an unknown node, unsafe name, oversized bytes or wrong digest
- WHEN artifact ingress/download runs
- THEN the service MUST reject without exposing or replacing unrelated content.

### Requirement: Exact authorized native controls

r[kiln.web.controls] Control admission MUST bind current canonical revision, active attempt, native execution and provider sequence and MUST check the selected capability before an external effect. Intent MUST be durable first. Ambiguous control MUST remain uncertain and recover through observation rather than automatic repeated mutation.

#### Scenario: Supported exact cancellation

r[kiln.web.controls.scenario.cancel]
- GIVEN an active native execution and authorized exact request
- WHEN cancellation is admitted
- THEN the existing workflow control port MUST receive that exact request and the UI MUST reflect admitted observations.

#### Scenario: Control is stale or unsupported

r[kiln.web.controls.scenario.denied]
- GIVEN a moved attempt/revision/sequence or unsupported control
- WHEN admission runs
- THEN no provider control effect MAY occur.

### Requirement: Honest live browser surface

r[kiln.web.interface] A dedicated live surface MUST expose enrollment, source-bound initiation, admitted run/node state, real logs/artifacts and supported controls, with visible authentication, stale/unknown/provider failures and bounded reconnect behavior. Untrusted facts MUST remain inert. Existing offline HTML MUST remain read-only without control authority or simulated hosted accounts.

#### Scenario: Operator follows a real run

r[kiln.web.interface.scenario.workflow]
- GIVEN an authenticated live browser
- WHEN a repository is enrolled and run
- THEN source, actual state/logs, real artifact downloads and exact controls MUST be operable end-to-end.

#### Scenario: Browser loses authority or stream data

r[kiln.web.interface.scenario.loss]
- GIVEN logout, rejected authority, unavailable provider or bounded stream loss
- WHEN the interface updates
- THEN it MUST clear private session state or display the precise limit/failure without optimistic success.

### Requirement: Repeatability and bounded evidence

r[kiln.web.evidence] Kiln maintainers MUST retain paired admission/durability tests and actual isolated native HTTPS/browser evidence covering enrollment, source-bound execution, logs, artifact identity, stale/unsupported controls, cancellation and restart. Documentation MUST name current consumer, maintenance owner, reproduction inputs and claim limits. Passing checks MUST NOT imply production activation, hostile isolation, source trust, lifecycle acceptance, deployment success or release eligibility.

#### Scenario: Isolated exercise completes

r[kiln.web.evidence.scenario.runtime]
- GIVEN disposable owned source/state/workflow resources
- WHEN the repeatable exercise runs
- THEN measured source/profile/executable identities and actual outcomes MUST be retained and temporary authority/resources retired.

#### Scenario: Stronger claim is proposed

r[kiln.web.evidence.scenario.scope]
- GIVEN passing checks and local runtime evidence
- WHEN production, source trust, confinement or release readiness is claimed
- THEN the evidence MUST retain its explicit non-claim boundary.

### Requirement: Lattice workflow authoring

r[kiln.sdlc.lattice_authoring]
Kiln MUST view and edit owner-admitted Lattice workflow definitions from Kiln. Forward through a configured Lattice authoring boundary. Display immutable revisions and source; submit a validated draft against the expected head. Preserve validation receipts and show stale or unsupported commands without publishing or executing automatically.

#### Scenario: Operator uses lattice workflow authoring

r[kiln.sdlc.lattice_authoring.scenario.operator]
- GIVEN an authenticated operator and an explicitly configured provider
- WHEN they edit a saved workflow from a known revision
- THEN Kiln MUST validate the draft through Lattice and save a new revision only against its expected head

### Requirement: Exact-revision supersession visibility and explicit Unknown run reconciliation

r[kiln.web.run_supersession_and_reconcile]
The live web service MUST expose `POST /api/runs/{id}/reconcile` delegating to `Engine::refresh` so operators can explicitly reconcile `Unknown` runs without blindly issuing a replacement rerun. The run list and run detail views in `web.js` MUST render serialized `superseded_by`, `superseded_revision`, and `dirty_snapshot` facts with an explicit invalidation or drift notice and a direct jump to the superseding run when present.

#### Scenario: Reconciling an Unknown run

r[kiln.web.run_supersession_and_reconcile.scenario.unknown]
- GIVEN a retained run whose outcome is `Unknown`
- WHEN the operator opens the run and invokes the Reconciliation Card action (`POST /api/runs/{id}/reconcile`)
- THEN the service MUST run provider reconciliation via `Engine::refresh` without creating a replacement run
- AND the UI MUST display the reconciliation result or retain `Unknown` honestly when the provider remains unreachable

#### Scenario: Superseded or dirty-snapshot run shows explicit binding drift

r[kiln.web.run_supersession_and_reconcile.scenario.superseded]
- GIVEN a retained run with `superseded_by` or `dirty_snapshot` set in `RunView`
- WHEN the operator views the run in the list or detail panel
- THEN the console MUST visibly mark the run as superseded or dirty-snapshot and link directly to `superseded_by`

### Requirement: Preserve authenticated sessions across browser pagehide

r[kiln.web.session_continuity_pagehide]
The console MUST NOT invoke `logout()` from the `pagehide` event handler. Explicit operator sign-out MUST still clear in-memory credentials, and bearer tokens MUST continue to avoid URL parameters and persistent `localStorage`.

#### Scenario: Browser sleep or tab transition preserves operator session

r[kiln.web.session_continuity_pagehide.scenario.preserve]
- GIVEN an authenticated operator monitoring live Factory state
- WHEN the browser fires `pagehide` during sleep, navigation, or bfcache transition
- THEN the console MUST NOT automatically clear the session token or force re-authentication
- AND clicking explicit sign-out MUST still clear credentials immediately

### Requirement: Single multiplexed SSE event stream backed by a shared coordinator watcher

r[kiln.web.multiplexed_sse_events]
`kiln-web-service` MUST replace 1.5s CI short-polling (`/api/live`) and per-tab upstream Fleet polling loops (`/api/fleet/events`) with a single multiplexed persistent SSE stream per browser tab (`/api/events`) emitting typed `ci`, `fleet`, `cairn`, and `attention` invalidation/snapshot frames. A single shared background coordinator watcher inside `kiln-web-service` MUST fan out upstream `kiln-fleet` updates to all connected SSE clients so multiple browser tabs do not multiply upstream connections against `kiln-fleet`'s `MAX_CONNECTIONS = 32` limit or starve `kiln-web-service`'s `max_connections` bound.

#### Scenario: Multi-tab monitoring uses one upstream watcher and one SSE stream per tab

r[kiln.web.multiplexed_sse_events.scenario.fanout]
- GIVEN five browser tabs connected to `/api/events`
- WHEN `kiln-fleet` or the CI engine records a state transition
- THEN `kiln-web-service` MUST perform only one upstream observation loop and fan out the typed SSE frame to all five tabs
- AND closing a tab MUST release its SSE server thread promptly

### Requirement: Compressed, ETag-cached static console assets and Node module test coverage

r[kiln.web.static_asset_caching_and_compression]
`crates/kiln-web-service/src/server.rs` MUST serve static console assets with deterministic content-derived `ETag` headers, `304 Not Modified` handling for matching `If-None-Match` requests, bounded `Cache-Control` headers, and negotiated `gzip` or `zstd` compression with `Vary: Accept-Encoding`. The repository MUST provide Node module tests loading `web.js`, `fleet.js`, `work.js`, and `project.js` and record them in `docs/console-port-inventory.json`.

#### Scenario: Repeat console load hits 304 and cold load uses negotiated compression

r[kiln.web.static_asset_caching_and_compression.scenario.http_caching]
- GIVEN a browser requesting static JS/CSS console assets with `Accept-Encoding: gzip`
- WHEN the asset is fetched cold and then re-requested with its returned `ETag` in `If-None-Match`
- THEN the cold response MUST return compressed bytes with `ETag` and `Vary: Accept-Encoding`
- AND the conditional repeat request MUST return `304 Not Modified` with an empty body

### Requirement: Operator-guarded multi-forge repository import, mirror management, and aggregated catalog API

r[kiln.web.repository_import_api]
`kiln-web-service` MUST expose `GET /api/repositories` (Viewer or Operator; `403` for Worker) returning the reconciled repository catalog combining durably persisted imports, `service.json` CI projects, `sources.json` local checkouts, coordinator-local checkouts, and proxied Fleet local copies with explicit per-source health. It MUST expose Operator-only endpoints `POST /api/repositories/import`, `POST /api/repositories/{id}/mirrors`, `POST /api/repositories/{id}/remove`, and `POST /api/repositories/refresh` enforcing `request_id` replay/conflict idempotency, atomic catalog persistence in `state_directory`, SSE `EventHub` invalidation, and strict read-only Radicle discipline (importing or mirroring a Radicle RID MUST NOT run Radicle init, signing, push, sync, or mirror publication, and removing a catalog entry MUST NOT delete any files on disk).

#### Scenario: Importing GitHub, Gitea, and Radicle remotes and attaching mirrors

r[kiln.web.repository_import_api.scenario.import_and_mirror]
- GIVEN an authenticated Operator importing a canonical Radicle RID and attaching GitHub and Gitea mirrors via `POST /api/repositories/import` and `POST /api/repositories/{id}/mirrors`
- WHEN `GET /api/repositories` is queried
- THEN the repository MUST appear under one stable BLAKE3 Kiln identity with all three remotes carrying their exact `canonical` or `mirror` roles
- AND replaying the same `request_id` with identical payload MUST be idempotent while conflicting payload reuse MUST fail with `409`
- AND a Viewer or Worker calling mutation endpoints MUST fail closed with `403`
