# Provider Ports Specification

## Purpose

Defines the `provider-ports` capability.

## Requirements

### Requirement: Provider-independent core
r[kiln.providers.core_independence] Kiln core crates MUST NOT depend on Aspen, Mantle, Lattice, provider SDKs, or shell-owned I/O and authority libraries.

#### Scenario: Core manifest imports a provider SDK
r[kiln.providers.core_independence.scenario.import]
- GIVEN a core manifest or source file that imports a provider implementation
- WHEN the dependency-boundary gate runs
- THEN the gate MUST fail before release admission.

### Requirement: Versioned provider contracts
r[kiln.providers.versioning] Every provider request, observation, capability profile, outcome, and external reference MUST name a supported contract version.

#### Scenario: Provider uses an unknown version
r[kiln.providers.versioning.scenario.unknown]
- GIVEN a provider profile with an unsupported contract version
- WHEN provider admission runs
- THEN the provider MUST be rejected without effect dispatch.

### Requirement: Typed external identities
r[kiln.providers.external_identity] Provider-native identities MUST include their provider namespace and native kind and MUST NOT replace primary Kiln identities.

#### Scenario: Two providers return equal text identifiers
r[kiln.providers.external_identity.scenario.namespace]
- GIVEN two providers that return the same native run identifier text
- WHEN Kiln records both observations
- THEN their external references MUST remain distinct and their Kiln attempts MUST remain distinct.

### Requirement: Composable narrow ports
r[kiln.providers.composition] Kiln MUST define separate ports for source, collaboration, trigger, status, workflow, build, event storage, notification, evidence, and gate effects.

#### Scenario: Deployment combines independent providers
r[kiln.providers.composition.scenario.mixed]
- GIVEN a deployment with Git source, Radicle triggers, Lattice workflows, and Mantle builds
- WHEN provider composition is admitted
- THEN each selected port MUST be checked independently without requiring one provider to implement all roles.

### Requirement: Capability admission before effects
r[kiln.providers.capabilities] Kiln MUST validate provider operation support, limits, identity namespace, cancellation behavior, usage facts, and outcome classes before effect dispatch.

#### Scenario: Build provider lacks cancellation
r[kiln.providers.capabilities.scenario.cancel]
- GIVEN a run policy that requires provider cancellation and a provider without that capability
- WHEN the run is admitted
- THEN admission MUST fail before a build request is dispatched.

### Requirement: No silent fallback
r[kiln.providers.no_fallback] Kiln MUST reject unsupported capabilities and unavailable selected providers instead of selecting another provider or weakening policy without an explicit new command.

#### Scenario: Selected workflow provider is unavailable
r[kiln.providers.no_fallback.scenario.unavailable]
- GIVEN a pipeline revision bound to one workflow provider
- WHEN that provider is unavailable at dispatch time
- THEN Kiln MUST record the bounded failure and MUST NOT select the standalone runner automatically.

### Requirement: Typed effect and completion linkage
r[kiln.providers.effects] Every provider dispatch and completion MUST bind the Kiln effect identity, attempt identity, provider profile, operation, idempotency key, and exact external references that are available.

#### Scenario: Completion names another attempt
r[kiln.providers.effects.scenario.mismatch]
- GIVEN a completion whose external run exists but whose Kiln attempt link is wrong
- WHEN completion admission runs
- THEN the completion MUST be rejected without changing either attempt.

### Requirement: Explicit ambiguous outcomes
r[kiln.providers.ambiguity] Provider contracts MUST distinguish success, domain failure, denial, cancellation, timeout, provider error, and unknown outcome without collapsing transport ambiguity into failure or success.

#### Scenario: Provider lookup cannot confirm submission
r[kiln.providers.ambiguity.scenario.lookup]
- GIVEN an earlier ambiguous dispatch and a failed reconciliation lookup
- WHEN the observation enters Kiln
- THEN the attempt MUST remain unknown unless explicit operator policy permits another classification.

### Requirement: Shared conformance suite
r[kiln.providers.conformance] Every provider adapter MUST pass the same positive and negative contract suite for each capability that it declares.

#### Scenario: Adapter declares status publication
r[kiln.providers.conformance.scenario.status]
- GIVEN an adapter that declares status publication
- WHEN conformance tests supply valid and malformed status requests
- THEN valid requests MUST map exactly and malformed requests MUST fail without publication.

### Requirement: Standalone reference profile
r[kiln.providers.standalone] Kiln MUST provide deterministic standalone fixture providers that exercise the full domain without Aspen, Mantle, or Lattice.

#### Scenario: Stack providers are absent
r[kiln.providers.standalone.scenario.absent]
- GIVEN no Aspen, Mantle, or Lattice installation
- WHEN the maintained standalone fixture runs
- THEN it MUST complete one accepted run and one rejected run using only reference providers.

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
