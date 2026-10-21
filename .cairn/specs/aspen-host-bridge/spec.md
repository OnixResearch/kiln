# Aspen Host Bridge Specification

## Purpose

Defines the `aspen-host-bridge` capability.

## Requirements

### Requirement: Exact Aspen host composition

r[kiln.aspen_host.composition] Kiln MUST publish a host bridge that serves its versioned Unix protocol through Aspen's reviewed native host and the exact Kiln extension executable.

#### Scenario: Exact host cohort starts

r[kiln.aspen_host.composition.accepted]
- GIVEN the reviewed Aspen profile, extension executable, socket, state root, and positive bounds
- WHEN the host bridge starts
- THEN it MUST admit the exact executable and profile before binding the service
- AND callback execution MUST use Aspen's bounded native-process port

#### Scenario: Host cohort drifts

r[kiln.aspen_host.composition.rejected]
- GIVEN a changed Aspen revision, profile identity, extension identity, executable bytes, protocol version, generation, or unsafe path
- WHEN startup admission runs
- THEN the bridge MUST fail before accepting a request
- AND it MUST NOT select a local simulation or another runtime

### Requirement: Bounded Unix server and reconciliation

r[kiln.aspen_host.transport] The host bridge MUST decode and encode bounded exact request frames, retain process-local operation acknowledgements, and reconcile without provider redispatch. The server MUST bound accepted connections and continue serving after a per-connection read, decode, encode, or response-write failure.

#### Scenario: Submit and reconcile agree

r[kiln.aspen_host.transport.accepted]
- GIVEN one valid request reaches the admitted socket
- WHEN submit completes and the client later reconciles its operation
- THEN both responses MUST bind the same profile, operation, run, and terminal observation
- AND a repeated submit MUST return replay without a second provider effect

#### Scenario: Frame or acceptance certainty fails

r[kiln.aspen_host.transport.rejected]
- GIVEN an oversized, malformed, stale, unsupported, incomplete, or conflicting frame
- WHEN the server handles the connection
- THEN it MUST reject without a provider effect
- AND a disconnect after possible acceptance MUST remain `Unknown`

#### Scenario: Client disconnect does not stop the host

r[kiln.aspen_host.transport.disconnect]
- GIVEN a client connects, sends a valid request, and disconnects before reading the response
- WHEN the server writes the response and the write fails
- THEN it MUST log the failure and consume one request slot
- AND later valid requests MUST still be served within the connection bound

### Requirement: Concrete Lattice workflow effect adapter

r[kiln.aspen_host.lattice] The host bridge MUST route the admitted hosted workflow effect through Kiln's pinned Lattice adapter and exact Unix workflow exchange.

#### Scenario: Linked workflow completes

r[kiln.aspen_host.lattice.accepted]
- GIVEN the effect request names a stored exact trigger and the selected Lattice profile
- WHEN Lattice accepts and returns a terminal observation
- THEN the adapter MUST preserve the generated workflow plan and observation linkage
- AND it MUST return one bounded typed provider completion

#### Scenario: Provider request cannot be linked

r[kiln.aspen_host.lattice.rejected]
- GIVEN the effect has a missing trigger, wrong provider profile, changed request bytes, unavailable socket, or conflicting replay state
- WHEN provider routing runs
- THEN routing MUST fail closed
- AND the host MUST NOT invoke another provider or direct executor

### Requirement: Exact terminal value mapping

r[kiln.aspen_host.completion] Kiln MUST derive hosted terminal meaning only from the exact materialized provider completion admitted by Aspen.

#### Scenario: Materialized terminal observation is valid

r[kiln.aspen_host.completion.accepted]
- GIVEN Aspen delivers a version-two completion containing matching bounded Lattice observation bytes
- WHEN the extension validates the completion
- THEN it MUST map the admitted provider outcome to the corresponding Kiln terminal state
- AND it MUST retain the exact provider observation identity

#### Scenario: Completion has no trustworthy value

r[kiln.aspen_host.completion.rejected]
- GIVEN the completion is legacy, reference-only, oversized, malformed, identity-mismatched, non-terminal, or linked to another effect
- WHEN callback completion handling runs
- THEN Kiln MUST reject it or retain `Unknown`
- AND it MUST NOT infer success from the output reference

### Requirement: Process-local canary claim boundary

r[kiln.aspen_host.nonclaims] Host receipts and documentation MUST identify the first bridge as process-local and MUST preserve runtime, durability, provider, deployment, and release non-claims.

#### Scenario: Host conformance passes

r[kiln.aspen_host.nonclaims.accepted]
- GIVEN production client/server and real Aspen callback tests pass
- WHEN evidence is recorded
- THEN it MAY claim exact local hosting and protocol conformance
- AND it MUST NOT claim active-operation recovery, production availability, CI correctness, workflow correctness, or release eligibility

### Requirement: Durable hosted-operation state

r[kiln.aspen_host.production_durability] The deployable host MUST persist each admitted ingress, exact trigger, Aspen native journal record, callback value, provider certainty state, and terminal acknowledgement before that fact is required after process loss.

#### Scenario: Terminal operation survives process loss

r[kiln.aspen_host.production_durability.accepted]
- GIVEN a hosted operation reached one admitted terminal acknowledgement
- WHEN the host process stops and reopens the same state root
- THEN a repeated submit or reconcile MUST return the exact acknowledgement
- AND it MUST NOT dispatch another provider effect

#### Scenario: Durable state is missing or changed

r[kiln.aspen_host.production_durability.rejected]
- GIVEN a record, callback value, journal entry, profile identity, generation, or content identity is missing, truncated, malformed, or substituted
- WHEN host recovery opens the state root
- THEN startup or the affected operation MUST fail closed
- AND the host MUST NOT infer a terminal result or safe retry

### Requirement: Exact recovery without redispatch

r[kiln.aspen_host.production_recovery] The host MUST classify a process-lost pending or uncertain provider operation from durable facts and MUST use the exact Lattice recovery or reconciliation operation before any terminal transition.

#### Scenario: Accepted operation completes after restart

r[kiln.aspen_host.production_recovery.accepted]
- GIVEN the provider accepted an exact operation before the host lost its process
- WHEN the restored host receives the same submit or reconcile request
- THEN it MUST use the original request, effect, provider, and idempotency identities
- AND it MUST recover or reconcile without calling provider dispatch again

#### Scenario: Provider acceptance cannot be established

r[kiln.aspen_host.production_recovery.unknown]
- GIVEN the durable ledger proves intent but Lattice cannot prove rejection or terminal completion
- WHEN recovery reaches its finite bound
- THEN Kiln MUST retain `Unknown`
- AND it MUST NOT retry, select another provider, or erase the uncertain record

### Requirement: Bounded hosted Radicle Nix provider

r[kiln.radicle.hosted_nix_provider] Kiln MUST provide one shell adapter that admits an exact typed profile and explicit Radicle source facts, runs only the fixed Nix wrapper through Bounded Exec, and publishes bounded deterministic reports.

#### Scenario: Exact revision completes

r[kiln.radicle.hosted_nix_provider.accepted]
- GIVEN the admitted user, repository, revision, source root, wrapper identity, report root, and positive limits
- WHEN Lattice starts the provider for one source-bound workflow node
- THEN the provider MUST execute only the admitted argv with a cleared child environment
- AND it MUST publish a bounded log and JSON report bound to the repository, revision, command, outcome, and BLAKE3 run identity

#### Scenario: Provider input or execution escapes policy

r[kiln.radicle.hosted_nix_provider.rejected]
- GIVEN a wrong user, repository, revision shape, relative or escaping path, changed command, inherited environment, timeout, cancellation, output flood, or report conflict
- WHEN provider admission or execution runs
- THEN it MUST fail without false success
- AND every owned child process MUST be terminated within the named teardown bound

### Requirement: Production-host evidence boundary

r[kiln.aspen_host.production_evidence] Production eligibility evidence MUST cover real-process durability, recovery, corruption, provider teardown, finite load, backup/restore, explicit rollback, authority, and exact publication identities.

#### Scenario: Production evidence is accepted

r[kiln.aspen_host.production_evidence.accepted]
- GIVEN every focused and broad gate passes for the immutable cohort
- WHEN the release receipt is evaluated
- THEN it MUST link the exact source, executable, profile, host, provider, restart, corruption, load, backup, and rollback evidence
- AND it MAY claim only the bounded local production-host behavior that was observed

#### Scenario: Local durability is promoted into a global claim

r[kiln.aspen_host.production_evidence.rejected]
- GIVEN only machine-local journal, ledger, callback value, workflow, and report observations
- WHEN evidence claims global durability, exactly-once delivery, CI correctness, external provider truth, host sandboxing, or release eligibility by implication
- THEN validation MUST reject the stronger claim

### Requirement: Exact extension runtime profile binding

r[kiln.aspen_host.extension_profile_binding] The production Aspen host MUST bind every extension callback execution to the same exact admitted runtime profile used for host ingress.

#### Scenario: Matching non-default profile is accepted

r[kiln.aspen_host.extension_profile_binding.accepted]
- GIVEN the host admits an explicit runtime profile with a non-default positive bound
- AND the host supplies its canonical path and exact BLAKE3 profile identity to the extension process
- WHEN the extension admits the same profile document
- THEN it MUST handle the callback with that admitted profile
- AND lifecycle, trigger, and recovery meaning MUST use one profile identity

#### Scenario: Profile input is absent or differs

r[kiln.aspen_host.extension_profile_binding.rejected]
- GIVEN the profile arguments or document are missing, duplicated, malformed, unreadable, oversized, invalid, changed, or identity-mismatched
- WHEN extension shell admission runs
- THEN execution MUST fail before callback handling
- AND it MUST NOT select the compiled fixture, another profile, or another runtime
- AND an already reserved operation MUST remain `Unknown`
- AND reconciliation MUST NOT redispatch that operation

### Requirement: Paced bounded Lattice observation

r[kiln.aspen_host.lattice_poll_pacing] The production Aspen host MUST use one explicit positive interval between non-terminal Lattice observations while preserving the existing finite poll bound.

#### Scenario: Delayed workflow reaches a terminal observation

r[kiln.aspen_host.lattice_poll_pacing.accepted]
- GIVEN Lattice accepts one workflow and first returns a non-terminal observation
- AND the host has a positive observation interval and finite poll count
- WHEN Lattice reaches a terminal observation before that count is exhausted
- THEN the host MUST wait between observe calls
- AND it MUST persist the exact terminal observation without provider redispatch

#### Scenario: Pacing is invalid or exhausted

r[kiln.aspen_host.lattice_poll_pacing.rejected]
- GIVEN the observation interval is missing, duplicated, zero, malformed, or the finite poll count expires
- WHEN host admission or workflow routing runs
- THEN invalid admission MUST fail before the host binds its socket
- AND bound exhaustion MUST remain `Unknown`
- AND recovery MUST use `RecoverDispatch` and observation without normal redispatch
