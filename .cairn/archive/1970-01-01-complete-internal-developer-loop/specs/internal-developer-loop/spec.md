# Internal Developer Loop

## ADDED Requirements

### Requirement: Real pipeline observations

r[kiln.web.developer_loop.pipeline]
The service MUST support a real multi-node dependency workflow with observed step/node timing, measured resource facts and retained structured reports. Unknown historical facts MUST remain unknown.

#### Scenario: Pipeline success and failure

- GIVEN a reviewed multi-node workflow
- WHEN actual commands execute
- THEN dependency ordering and failures are preserved and the inspector presents source-bound observed timing/resource/report facts rather than invented progress.

### Requirement: Worker execution contract

r[kiln.web.developer_loop.workers]
Workers MUST publish authenticated bounded liveness/resource observations distinct from configured admission slots. Selected isolation MUST fail closed when unavailable, preserve bounded cancellation/timeout teardown, and retain exact recovery identity.

#### Scenario: Stale and unavailable execution

- GIVEN a stale heartbeat or unavailable selected isolation backend
- WHEN execution admission is evaluated
- THEN stale observations do not claim online capacity and no unsandboxed fallback executes.

### Requirement: Shared cache

r[kiln.web.developer_loop.cache]
Workers MUST restore and publish bounded provider-backed shared cache entries with declared input/producer/platform keys, verified BLAKE3 content, safe paths, explicit retention policy and actual hit/miss/transfer observations.

#### Scenario: Warm and corrupt cache

- GIVEN an admitted cold run then a matching warm run
- WHEN the second restores the cache
- THEN actual transferred bytes and hit/miss facts are recorded; corrupt or unsafe content is rejected before restoration.

### Requirement: Developer CLI and dirty inputs

r[kiln.web.developer_loop.cli]
The developer CLI MUST submit, watch, read logs, cancel and retry real executions through authenticated bounded APIs. Explicit dirty-worktree submissions MUST have exact content identity and be distinguishable from committed-ref runs, without publishing source or disclosing credentials.

#### Scenario: Exact dirty input

- GIVEN modified local source and explicit dirty input selection
- WHEN submitted and executed
- THEN the run observes exactly the admitted snapshot, retains its provenance and identity, and does not report the original committed tree as the executed input. Invalid input or stale identities MUST fail before effects.
