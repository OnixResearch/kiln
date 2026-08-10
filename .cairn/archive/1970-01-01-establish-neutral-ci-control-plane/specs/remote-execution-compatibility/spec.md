# Remote Execution Compatibility Specification

## ADDED Requirements

### Requirement: Optional remote-execution adapter
r[kiln.remote.optional] Remote Execution API compatibility MUST live behind optional build and log adapter ports and MUST NOT add protocol types, clients, transport, cache, or execution dependencies to Kiln core.

#### Scenario: Remote-execution adapter is absent
r[kiln.remote.optional.scenario.absent]
- GIVEN a standalone Kiln profile without a Remote Execution API implementation
- WHEN core and standalone conformance run
- THEN they MUST pass without remote-execution software or generated protocol code.

### Requirement: Distinct remote identities
r[kiln.remote.identities] Kiln MUST preserve content digest, input root, command, action, operation, execution, cache result, log stream, worker, and provider identities as distinct external facts linked to one Kiln effect and attempt.

#### Scenario: Cache result and operation share text
r[kiln.remote.identities.scenario.equal_text]
- GIVEN a provider that returns equal text for a cache result and an operation
- WHEN Kiln records both references
- THEN their native kinds MUST keep them distinct.

### Requirement: Capability admission before execution
r[kiln.remote.capabilities] The adapter MUST observe and admit protocol version, digest functions, execution support, cache support, symlink policy, compression, limits, cancellation, log streaming, and outcome behavior before dispatch.

#### Scenario: Provider lacks required digest function
r[kiln.remote.capabilities.scenario.digest]
- GIVEN an action profile that requires a digest function absent from provider capabilities
- WHEN build admission runs
- THEN it MUST fail before content upload or execution request.

### Requirement: Exact action binding
r[kiln.remote.action] A remote action request MUST bind the exact Kiln pipeline revision, candidate or source snapshot, command, environment, platform properties, input root, declared outputs, timeout policy, provider profile, and producing attempt.

#### Scenario: Input root changes after admission
r[kiln.remote.action.scenario.input_drift]
- GIVEN an admitted action and different observed input-root bytes
- WHEN dispatch preflight runs
- THEN it MUST reject the stale action before upload or execution.

### Requirement: Native digest interoperability
r[kiln.remote.digests] Protocol-required digest algorithms and encodings MUST remain native external identities, while Kiln-owned canonical manifests and artifacts retain separate BLAKE3 identities.

#### Scenario: CAS uses a protocol digest
r[kiln.remote.digests.scenario.native]
- GIVEN content addressed by a provider-required digest
- WHEN Kiln links the CAS object to a canonical product
- THEN both identities MUST remain explicit and neither MUST replace the other.

### Requirement: Explicit operation lifecycle
r[kiln.remote.operations] Execute, poll, wait, cancel, cache lookup, upload, download, and log operations MUST map to versioned requested effects and observations without collapsing accepted, queued, executing, completed, absent, expired, denied, provider-error, or unknown states.

#### Scenario: Wait transport fails after acceptance
r[kiln.remote.operations.scenario.wait_failure]
- GIVEN a provider operation that was accepted before the wait connection failed
- WHEN the adapter reports the transport failure
- THEN the Kiln attempt MUST remain unknown or reconciling and MUST NOT become not-started.

### Requirement: Cache observations are not execution proof
r[kiln.remote.cache] A remote cache hit or miss MUST remain a provider observation linked to the exact action and MUST NOT by itself prove execution, product correctness, source trust, or release eligibility.

#### Scenario: Cache hit lacks expected outputs
r[kiln.remote.cache.scenario.missing_output]
- GIVEN a cache hit that omits an output required by the admitted action
- WHEN completion admission runs
- THEN it MUST reject successful completion and retain the cache observation for diagnostics.

### Requirement: Bounded remote logs
r[kiln.remote.logs] Remote log streams MUST use named byte, record, duration, ordering, truncation, redaction, retention, and retrieval policies and MUST remain non-authoritative projections.

#### Scenario: Log exceeds admitted byte bound
r[kiln.remote.logs.scenario.truncate]
- GIVEN a remote log stream larger than the admitted bound
- WHEN the shell retains the stream
- THEN it MUST apply the declared truncation result and MUST NOT alter attempt outcome.

### Requirement: Remote-execution fixtures
r[kiln.remote.fixtures] Maintained adapter tests MUST include valid capability, upload, execute, cache, poll, cancel, completion, and log cases plus invalid version, digest, action, identity, output, timeout, transport, unknown-outcome, and cache-confusion cases.

#### Scenario: Unknown-outcome fixture is absent
r[kiln.remote.fixtures.scenario.missing]
- GIVEN a release candidate without a current accepted-then-disconnected fixture
- WHEN remote-execution readiness runs
- THEN the candidate MUST fail the adapter fixture gate.

### Requirement: Remote-execution non-claims
r[kiln.remote.nonclaims] Passing adapter checks MUST NOT claim remote service correctness, hermeticity, sandboxing, cache integrity, worker identity truth, execution completion, product correctness, or release eligibility.

#### Scenario: Provider reports success
r[kiln.remote.nonclaims.scenario.success]
- GIVEN an admitted successful remote operation observation
- WHEN Kiln reports the attempt
- THEN it MUST identify the provider observation and retain the remote-execution claim boundary.
