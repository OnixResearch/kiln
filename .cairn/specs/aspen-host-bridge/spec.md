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

r[kiln.aspen_host.transport] The host bridge MUST decode and encode bounded exact request frames, retain process-local operation acknowledgements, and reconcile without provider redispatch.

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
