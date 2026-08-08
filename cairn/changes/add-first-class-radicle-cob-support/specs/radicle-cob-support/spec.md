# First-Class Radicle COB Support Specification

## ADDED Requirements

### Requirement: Typed COB identity model
r[kiln.radicle_cobs.model] Kiln MUST model a Radicle collaborative object with separate repository, type, object, operation, semantic-state, and canonical Kiln identities.

#### Scenario: Provider object enters Kiln
r[kiln.radicle_cobs.model.scenario.identity]
- GIVEN a supported Radicle collaborative object with a repository identity, type name, object ID, and operation heads
- WHEN the Radicle adapter maps the object into Kiln
- THEN it MUST preserve each native identity as a typed external fact
- AND it MUST NOT use a provider object or operation ID as a primary Kiln identity.

#### Scenario: Equal semantic state has different operation history
r[kiln.radicle_cobs.model.scenario.history]
- GIVEN two reduced objects with equal admitted semantic state and different operation histories
- WHEN Kiln compares the observations
- THEN it MAY record equal semantic content
- AND it MUST NOT claim equal history, signatures, authority, or provenance.

### Requirement: Versioned COB profile registry
r[kiln.radicle_cobs.profile_registry] Kiln MUST use a typed versioned registry that declares each supported COB type, schema line, semantic mapper, capabilities, bounds, conflict behavior, and claim boundary.

#### Scenario: Registered profile is admitted
r[kiln.radicle_cobs.profile_registry.scenario.supported]
- GIVEN a COB type and schema line that match one enabled profile
- WHEN provider admission runs
- THEN Kiln MUST select that exact profile and expose only its declared read, trigger, projection, or mutation capabilities.

#### Scenario: Unknown type requests semantic mapping
r[kiln.radicle_cobs.profile_registry.scenario.unknown]
- GIVEN an unregistered COB type or schema line
- WHEN an adapter requests semantic mapping or mutation
- THEN Kiln MUST reject the request before external effects
- AND it MAY retain a bounded opaque observation only under an explicit read-only policy.

### Requirement: Initial Radicle and Cairn profiles
r[kiln.radicle_cobs.initial_profiles] The initial registry MUST cover supported native Radicle issue, patch, and CI job profiles plus the pinned Cairn change, RFD, ADR, verification-obligation, release-decision, and policy-exception profiles.

#### Scenario: Cairn ADR is observed
r[kiln.radicle_cobs.initial_profiles.scenario.adr]
- GIVEN a valid `xyz.onixresearch.adr` object under the pinned Cairn contract
- WHEN the Cairn ADR profile maps its reduced state
- THEN Kiln MUST preserve the ADR identity, lifecycle, current heads, role observations, lineage, and exact external references
- AND it MUST NOT treat ADR acceptance as a Kiln command or provider effect.

#### Scenario: Supported profiles are incomplete
r[kiln.radicle_cobs.initial_profiles.scenario.missing]
- GIVEN a candidate registry that omits a required initial profile or exact format pin
- WHEN COB adapter readiness runs
- THEN the candidate MUST fail before the profile set becomes available.

### Requirement: Provider semantics stay outside the core
r[kiln.radicle_cobs.semantic_boundary] Radicle and Cairn implementations, reducers, Git storage, signatures, profiles, nodes, and transports MUST remain outside Kiln core crates.

#### Scenario: Core receives a reduced observation
r[kiln.radicle_cobs.semantic_boundary.scenario.reduced]
- GIVEN a shell that validates and reduces a supported provider object
- WHEN the shell calls the pure Kiln mapping core
- THEN the core MUST receive bounded typed observations without repository handles, provider SDK values, credentials, or raw transport state.

#### Scenario: Core imports a provider implementation
r[kiln.radicle_cobs.semantic_boundary.scenario.import]
- GIVEN a core manifest or source file that imports Radicle, `radicle-cob`, Cairn, Git storage, or provider runtime code
- WHEN the dependency boundary gate runs
- THEN the gate MUST fail and identify the forbidden edge.

### Requirement: Separate observation, trigger, and mutation capabilities
r[kiln.radicle_cobs.capabilities] COB discovery, typed loading, event intake, status projection, and mutation MUST use separate versioned requests, observations, and capabilities.

#### Scenario: Read-only adapter observes an object
r[kiln.radicle_cobs.capabilities.scenario.read_only]
- GIVEN a profile with typed-read capability and no mutation capability
- WHEN the adapter loads and reduces one object
- THEN it MAY emit a bounded observation
- AND it MUST NOT request signing, update, conflict resolution, publication, or synchronization.

#### Scenario: Event grants mutation authority
r[kiln.radicle_cobs.capabilities.scenario.event_authority]
- GIVEN a valid signed COB operation event
- WHEN the trigger shell maps that event into Kiln
- THEN the event MAY request an admitted Kiln command
- AND it MUST NOT grant provider mutation, execution, merge, release, or policy-exception authority.

### Requirement: Preview-first COB effects
r[kiln.radicle_cobs.effects] Every COB mutation MUST start as a typed effect plan bound to one profile, repository, object, expected heads, action, actor reference, policy, and idempotency key.

#### Scenario: Fresh preflight passes
r[kiln.radicle_cobs.effects.scenario.fresh]
- GIVEN an admitted mutation plan with current heads, supported action, explicit credentials, and matching capability facts
- WHEN the shell performs fresh preflight
- THEN it MAY execute the exact provider action
- AND it MUST record the provider result without claiming that planning caused execution.

#### Scenario: Object advanced after planning
r[kiln.radicle_cobs.effects.scenario.stale]
- GIVEN a mutation plan whose expected heads differ from the current object heads
- WHEN fresh preflight runs
- THEN the shell MUST reject the stale plan without signing or writing an operation.

### Requirement: Explicit COB conflict state
r[kiln.radicle_cobs.conflicts] Kiln MUST preserve concurrent heads, reducer conflicts, malformed histories, and unresolved semantic conflicts as explicit typed observations.

#### Scenario: Concurrent artifact updates exist
r[kiln.radicle_cobs.conflicts.scenario.concurrent]
- GIVEN a supported object with concurrent current heads for one semantic field or artifact
- WHEN the adapter reports current state
- THEN Kiln MUST retain every bounded head and mark the affected semantic projection conflicted
- AND it MUST NOT select a winner by time, arrival order, actor, or object ID.

#### Scenario: Resolution omits a current head
r[kiln.radicle_cobs.conflicts.scenario.partial]
- GIVEN a profile that requires complete head selection for resolution
- WHEN a requested resolution omits a current head
- THEN the pure admission decision MUST reject the request before signing or storage mutation.

### Requirement: Exact applicability binding
r[kiln.radicle_cobs.applicability] Any COB fact used by a Kiln gate, evaluation, policy, or release decision MUST bind the exact semantic state, candidate, claim, policy, and provider observation identities that make it applicable.

#### Scenario: Release decision matches the candidate
r[kiln.radicle_cobs.applicability.scenario.release]
- GIVEN a valid Cairn release-decision object whose immutable candidate root matches one exact Kiln candidate
- WHEN Kiln evaluates an admitted release policy
- THEN it MAY use the reduced decision as one bounded external input
- AND it MUST keep release execution and Kiln readiness authority separate.

#### Scenario: Bound claim advances
r[kiln.radicle_cobs.applicability.scenario.advance]
- GIVEN a verification obligation or release decision whose bound claim, candidate, policy, or semantic state has advanced
- WHEN Kiln checks prior applicability
- THEN the prior result MUST become inapplicable without deleting its history.

### Requirement: Status COBs remain projections
r[kiln.radicle_cobs.status_projection] Radicle CI job COBs and other status COBs MUST remain rebuildable provider projections that cannot replace the Kiln journal.

#### Scenario: Status object disagrees with canonical state
r[kiln.radicle_cobs.status_projection.scenario.disagreement]
- GIVEN a Radicle status COB whose terminal result conflicts with admitted Kiln events and provider observations
- WHEN Kiln rebuilds canonical state
- THEN Kiln MUST retain the disagreement as an external observation
- AND it MUST NOT mutate canonical state from the status object.

### Requirement: Typed Nickel profile configuration
r[kiln.radicle_cobs.configuration] Human-authored COB profiles MUST use typed Nickel contracts with named limits, explicit capabilities, supported actions, format pins, and non-claims.

#### Scenario: Mutation bound is absent
r[kiln.radicle_cobs.configuration.scenario.bound]
- GIVEN a profile that enables mutation without a named action, object, head, payload, or retry bound
- WHEN configuration admission runs
- THEN the profile MUST fail before adapter startup.

#### Scenario: Profile silently changes format
r[kiln.radicle_cobs.configuration.scenario.drift]
- GIVEN a profile whose observed type, schema, reducer, or upstream revision differs from its admitted pin
- WHEN adapter admission runs
- THEN the adapter MUST fail closed and report the mismatched format fact.

### Requirement: Independent interoperability evidence
r[kiln.radicle_cobs.interop] Maintained COB conformance MUST include positive and negative fixtures from pinned upstream or independent producers and consumers.

#### Scenario: Supported Cairn object round-trips
r[kiln.radicle_cobs.interop.scenario.cairn]
- GIVEN a provenance-bound fixture for each supported Cairn COB profile
- WHEN the Kiln adapter reads, maps, exports, and rereads the fixture
- THEN it MUST preserve exact provider identities and admitted semantic facts without importing provider authority into Kiln.

#### Scenario: Malformed or mixed object enters the adapter
r[kiln.radicle_cobs.interop.scenario.malformed]
- GIVEN a wrong-type, malformed-history, forged-actor, stale-head, mixed-schema, oversized, or conflicting fixture
- WHEN conformance runs
- THEN the adapter MUST reject or explicitly classify the object without false semantic success.

### Requirement: COB support non-claims
r[kiln.radicle_cobs.nonclaims] Passing COB checks MUST NOT claim remote synchronization, peer convergence, actor trust, signature policy sufficiency, semantic correctness, evidence truth, execution success, merge authority, release eligibility, or future format compatibility.

#### Scenario: Local signed object passes
r[kiln.radicle_cobs.nonclaims.scenario.local]
- GIVEN a locally reduced signed object that passes its configured profile
- WHEN Kiln emits compatibility evidence
- THEN the evidence MUST remain limited to the checked bytes, identities, reduction, mapping, policy, and local provider observations.
