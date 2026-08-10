# Pijul Source Adapter Delta Specification

## ADDED Requirements

### Requirement: Pijul source adapter
r[kiln.source.pijul_adapter] Kiln MUST provide a versioned Pijul source adapter with a concrete read-only shell for admitted local repositories.

#### Scenario: Supported channel resolves
r[kiln.source.pijul_adapter.scenario.resolve]
- GIVEN an admitted local Pijul repository and one supported channel
- WHEN the Pijul adapter resolves and acquires the channel
- THEN it MUST return an exact native state, bounded Pijul facts, and one canonical tree identity
- AND it MUST NOT invent a Git commit or branch.

#### Scenario: Adapter has no concrete shell
r[kiln.source.pijul_adapter.scenario.shell]
- GIVEN a candidate adapter that contains only model types, traits, or fixture implementations
- WHEN adapter readiness runs
- THEN the candidate MUST fail before Pijul support appears in the implemented adapter inventory.

### Requirement: Exact Pijul state identity
r[kiln.source.pijul_state_identity] A Pijul resolution MUST preserve the repository, requested channel, provider-produced immutable state identity, bounded native state facts, format cohort, and observation identity.

#### Scenario: Channel state is stable
r[kiln.source.pijul_state_identity.scenario.stable]
- GIVEN a channel whose state stays unchanged during resolution and acquisition
- WHEN the adapter creates a source resolution
- THEN the exact revision MUST identify that native state
- AND the symbolic reference MUST remain the requested channel.

#### Scenario: Channel moves during acquisition
r[kiln.source.pijul_state_identity.scenario.moving]
- GIVEN a channel whose observed state changes across the resolution and acquisition boundary
- WHEN the adapter checks state consistency
- THEN it MUST reject the mixed observation without returning partial success.

#### Scenario: One change hash is used as channel state
r[kiln.source.pijul_state_identity.scenario.incomplete]
- GIVEN a channel state that requires more than one native fact
- WHEN a candidate mapping uses one change hash as the complete state identity
- THEN pure admission MUST reject the incomplete mapping.

### Requirement: Canonical Pijul snapshot
r[kiln.source.pijul_snapshot] The adapter MUST compute the canonical BLAKE3 tree identity from bounded snapshot bytes acquired for the exact admitted Pijul state.

#### Scenario: Snapshot is acquired
r[kiln.source.pijul_snapshot.scenario.acquire]
- GIVEN a stable admitted Pijul state with supported entries
- WHEN the shell acquires the snapshot
- THEN a pure function MUST validate and order the entries before canonical manifest framing
- AND the shell MUST compute the BLAKE3 identity from that manifest.

#### Scenario: Snapshot contains an unsupported entry
r[kiln.source.pijul_snapshot.scenario.entry]
- GIVEN a snapshot with an unsafe path, duplicate path, unsupported entry kind, or exceeded named bound
- WHEN snapshot admission runs
- THEN the adapter MUST reject the snapshot without emitting a tree identity.

#### Scenario: Equal trees have different Pijul history
r[kiln.source.pijul_snapshot.scenario.history]
- GIVEN two admitted Pijul states with equal canonical tree identities and different native state facts
- WHEN Kiln compares the resolutions
- THEN it MAY record equal source bytes
- AND it MUST NOT claim equal Pijul history, signatures, or trust.

### Requirement: Read-only Pijul capabilities
r[kiln.source.pijul_capabilities] The first Pijul profile MUST support only discovery, exact resolution, acquisition, and identity observation.

#### Scenario: Source acquisition is admitted
r[kiln.source.pijul_capabilities.scenario.read]
- GIVEN the selected Pijul profile and a bounded acquisition request
- WHEN provider admission runs
- THEN the profile MAY admit local read access for the selected repository and channel.

#### Scenario: Mutation is requested
r[kiln.source.pijul_capabilities.scenario.mutation]
- GIVEN the read-only Pijul profile
- WHEN a request attempts to record, apply, unrecord, push, pull, clone, or mutate a channel
- THEN provider admission MUST reject the request before repository or network effects.

### Requirement: Typed Pijul profile configuration
r[kiln.source.pijul_configuration] Human-authored Pijul profiles MUST use typed Nickel contracts with exact compatibility facts, explicit capabilities, named bounds, supported entry kinds, and non-claims.

#### Scenario: Compatible profile starts
r[kiln.source.pijul_configuration.scenario.compatible]
- GIVEN a complete profile whose Pijul version, library version, source revision, and repository format match the adapter
- WHEN profile admission runs
- THEN the adapter MAY make its declared read-only capabilities available.

#### Scenario: Compatibility fact drifts
r[kiln.source.pijul_configuration.scenario.drift]
- GIVEN a profile with a missing bound or mismatched version, revision, format, platform, or capability
- WHEN profile admission runs
- THEN it MUST fail before the shell opens a repository.

### Requirement: Pijul adapter conformance
r[kiln.source.pijul_conformance] Maintained conformance MUST include positive and negative repositories created or read by the pinned Pijul cohort.

#### Scenario: Native fixture resolves repeatedly
r[kiln.source.pijul_conformance.scenario.repeat]
- GIVEN a positive local repository fixture with a stable channel
- WHEN the adapter resolves and acquires it more than once
- THEN each result MUST retain equal native state facts and equal canonical tree identities.

#### Scenario: Repository is malformed or exceeds a bound
r[kiln.source.pijul_conformance.scenario.reject]
- GIVEN a missing channel, missing change, damaged state, unsupported format, unsafe entry, or exceeded named bound
- WHEN conformance runs
- THEN the adapter MUST reject or explicitly classify the input without false source success.

#### Scenario: Only handwritten fixtures exist
r[kiln.source.pijul_conformance.scenario.native]
- GIVEN a candidate whose fixtures were not produced or consumed by the pinned Pijul cohort
- WHEN adapter readiness runs
- THEN the candidate MUST fail the interoperability evidence gate.

### Requirement: Pijul adapter non-claims
r[kiln.source.pijul_nonclaims] Passing Pijul checks MUST NOT claim source trust, signature validity, complete history, remote availability, collaboration status, mutation success, or cross-provider history equivalence.

#### Scenario: Local snapshot passes
r[kiln.source.pijul_nonclaims.scenario.local]
- GIVEN a local Pijul repository whose channel resolves and acquires successfully
- WHEN Kiln records adapter evidence
- THEN the evidence MUST remain limited to the checked local state, bytes, profile, bounds, identities, and provider observations.

#### Scenario: Core or standalone imports Pijul
r[kiln.source.pijul_nonclaims.scenario.dependency]
- GIVEN a core or standalone manifest that imports Pijul provider runtime code
- WHEN the dependency boundary gate runs
- THEN the gate MUST fail and identify the forbidden dependency edge.
