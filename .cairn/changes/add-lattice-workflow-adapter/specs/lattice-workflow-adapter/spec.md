# Lattice Workflow Adapter Delta Specification

## ADDED Requirements

### Requirement: Versioned Lattice workflow profile
r[kiln.lattice.profile] Kiln MUST admit Lattice through a typed profile with exact contract, graph, capability, limit, and platform facts.

#### Scenario: Profile matches
r[kiln.lattice.profile.scenario.match]
- GIVEN a complete profile for the pinned Lattice cohort
- WHEN provider admission runs
- THEN Kiln MAY make the declared workflow capabilities available.

#### Scenario: Node kind is unsupported
r[kiln.lattice.profile.scenario.node]
- GIVEN a request that requires an undeclared node or effect kind
- WHEN capability admission runs
- THEN Kiln MUST reject it before dispatch.

### Requirement: Exact Lattice request linkage
r[kiln.lattice.request] Every Lattice request MUST bind one Kiln effect, attempt, candidate, pipeline revision, workflow policy, graph, and idempotency key.

#### Scenario: Request lowers deterministically
r[kiln.lattice.request.scenario.valid]
- GIVEN equal admitted Kiln inputs and equal profile facts
- WHEN pure request lowering runs
- THEN it MUST return equal bounded Lattice request values.

#### Scenario: Graph identity changes
r[kiln.lattice.request.scenario.graph]
- GIVEN a lowered request whose graph differs from its admitted identity
- WHEN request admission runs
- THEN Kiln MUST reject it before provider effects.

### Requirement: Lattice observation admission
r[kiln.lattice.observation] Lattice lifecycle and completion observations MUST preserve native identities and pass Kiln outcome admission.

#### Scenario: Workflow completes
r[kiln.lattice.observation.scenario.complete]
- GIVEN a linked terminal workflow observation with required node results
- WHEN observation admission runs
- THEN Kiln MAY record the corresponding CI outcome.

#### Scenario: Provider reports paused as success
r[kiln.lattice.observation.scenario.pause]
- GIVEN a paused Lattice execution reported as terminal success
- WHEN observation admission runs
- THEN Kiln MUST reject the false terminal outcome.

### Requirement: Lattice reconciliation
r[kiln.lattice.reconciliation] Ambiguous Lattice operations MUST remain unknown until exact execution reconciliation supplies an admitted result.

#### Scenario: Dispatch disconnects
r[kiln.lattice.reconciliation.scenario.disconnect]
- GIVEN a workflow accepted before transport disconnects
- WHEN outcome classification runs
- THEN Kiln MUST retain unknown and the native execution identity.

#### Scenario: Result is stale
r[kiln.lattice.reconciliation.scenario.stale]
- GIVEN a reconciliation result for an earlier attempt
- WHEN admission runs
- THEN Kiln MUST reject it without changing the active attempt.

### Requirement: Lattice adapter conformance
r[kiln.lattice.conformance] Maintained conformance MUST include external producer fixtures and positive and negative lifecycle cases.

#### Scenario: Cancellation fixture is absent
r[kiln.lattice.conformance.scenario.cancel]
- GIVEN an adapter candidate without cancellation and ambiguity fixtures
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: Lattice adapter non-claims
r[kiln.lattice.nonclaims] Passing checks MUST NOT claim workflow correctness, host confinement, external effect success, credential freshness, or release readiness.

#### Scenario: Workflow passes
r[kiln.lattice.nonclaims.scenario.scope]
- GIVEN a passing workflow observation
- WHEN Kiln records evidence
- THEN it MUST remain scoped to the exact graph, request, profile, policy, and observations.
