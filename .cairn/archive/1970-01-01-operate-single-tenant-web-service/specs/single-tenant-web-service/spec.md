# Single-tenant live web service

## ADDED Requirements

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
