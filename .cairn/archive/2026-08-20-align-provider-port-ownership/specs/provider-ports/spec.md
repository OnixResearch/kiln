## ADDED Requirements

### Requirement: Application-owned provider contracts

r[kiln.providers.port_ownership] Kiln application code MUST own each genuine provider capability contract. Concrete adapter crates MUST implement those contracts and MUST NOT publish competing application ports.

#### Scenario: Adapter defines a competing provider port

r[kiln.providers.port_ownership.scenario.competing]
- GIVEN a concrete provider adapter publishes a repository, transport, signer, broker, or mutation contract that application code consumes directly
- WHEN the provider boundary gate runs
- THEN the gate MUST fail
- AND the capability MUST move to an application-owned contract or remain a private adapter detail.

### Requirement: Provider policy and effects remain separate

r[kiln.providers.policy_effect_split] Deterministic admission, ambiguity, freshness, transition, and outcome decisions MUST consume explicit values and return decisions or effect plans without calling provider capabilities.

#### Scenario: Provider observation requires a decision

r[kiln.providers.policy_effect_split.scenario.observation]
- GIVEN a shell obtains a bounded provider observation
- WHEN Kiln evaluates its applicability
- THEN a pure core function MUST decide admission from explicit state and policy
- AND the adapter MUST NOT decide canonical Kiln state or terminal meaning.

### Requirement: Provider errors keep their owner

r[kiln.providers.error_ownership] Domain rejection, application failure, and provider infrastructure failure MUST use distinct typed categories until the application shell translates them.

#### Scenario: Transport disconnects after acceptance

r[kiln.providers.error_ownership.scenario.disconnect]
- GIVEN a provider accepted an operation before transport disconnected
- WHEN the shell translates the infrastructure observation
- THEN the attempt MUST remain unknown or reconciling under core policy
- AND the infrastructure failure MUST NOT become generic domain rejection or confirmed non-execution.

### Requirement: Concrete provider selection is visible

r[kiln.providers.composition_root] Only an explicit application composition root MUST select concrete provider adapters.

#### Scenario: Core or adapter selects another provider

r[kiln.providers.composition_root.scenario.hidden_selection]
- GIVEN a selected provider is unavailable or unsupported
- WHEN dispatch cannot continue
- THEN core and adapter code MUST return the typed blocker
- AND they MUST NOT construct or select another provider implicitly.

### Requirement: Port-boundary conformance covers failure paths

r[kiln.providers.boundary_conformance] Each migrated port MUST have positive and negative core, shell, and adapter tests for call order, denial, malformed data, stale identity, timeout, cancellation, ambiguity, and provider failure.

#### Scenario: Negative boundary coverage is missing

r[kiln.providers.boundary_conformance.scenario.missing_negative]
- GIVEN a provider adapter has a successful conformance case but lacks a required failure class
- WHEN adapter readiness runs
- THEN readiness MUST fail with the missing class
- AND successful provider calls MUST NOT substitute for negative-path evidence.
