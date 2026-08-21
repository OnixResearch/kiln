# Source Collaboration Specification

## Purpose

Defines the `source-collaboration` capability.

## Requirements

### Requirement: Neutral source model
r[kiln.source.model] Kiln MUST model repositories, named references, source snapshots, proposed changes, source events, actors, and resolution observations without requiring universal commit, branch, or pull-request semantics.

#### Scenario: Pijul channel is resolved
r[kiln.source.model.scenario.pijul]
- GIVEN a Pijul source provider and one named channel
- WHEN Kiln requests an immutable source snapshot
- THEN the result MUST fit the source model without inventing a Git commit or branch.

#### Scenario: Jujutsu bookmark is resolved
r[kiln.source.model.scenario.jujutsu]
- GIVEN a Jujutsu source provider and one bookmark that resolves to one commit
- WHEN Kiln requests an immutable source snapshot
- THEN the result MUST record the symbolic bookmark, exact commit ID, change ID, backend kind, and observed operation identity.

### Requirement: Split source and collaboration ports
r[kiln.source.port_split] Source materialization, collaboration metadata, trigger intake, status publication, identity observation, and credential resolution MUST use separate capability contracts.

#### Scenario: Local Git has no forge
r[kiln.source.port_split.scenario.local_git]
- GIVEN a local Git source provider without review or hosted status APIs
- WHEN provider admission runs
- THEN source capability MAY pass while collaboration and status capabilities remain unsupported.

#### Scenario: Jujutsu has no forge authority
r[kiln.source.port_split.scenario.jujutsu]
- GIVEN a Jujutsu source provider without a separate collaboration provider
- WHEN provider admission runs
- THEN source capability MAY pass while collaboration and hosted status capabilities remain unsupported.

### Requirement: Immutable source resolution
r[kiln.source.resolution] An evaluation MUST bind the original symbolic reference, exact provider-native revision, and exact canonical source snapshot before workflow or build effects.

#### Scenario: Main moves after evaluation admission
r[kiln.source.resolution.scenario.moving]
- GIVEN an admitted evaluation that resolved `main` to one exact revision
- WHEN the provider later moves `main`
- THEN the existing evaluation MUST retain the earlier revision and MUST NOT resolve `main` again.

#### Scenario: Jujutsu change is rewritten after admission
r[kiln.source.resolution.scenario.jujutsu_rewrite]
- GIVEN an admitted evaluation bound to one Jujutsu change ID and exact commit ID
- WHEN the provider rewrites that change to a new commit ID
- THEN the existing evaluation MUST retain the earlier commit ID and MUST NOT follow the rewritten change.

### Requirement: Canonical tree identity
r[kiln.source.tree_identity] A materialized source snapshot MUST include a canonical manifest and BLAKE3 tree identity computed from observed bytes by the source shell.

#### Scenario: Native revision returns different bytes
r[kiln.source.tree_identity.scenario.mismatch]
- GIVEN a recorded native revision and expected canonical tree identity
- WHEN later materialization produces different canonical bytes
- THEN source admission MUST fail with a mismatch observation.

### Requirement: Preserve native semantics
r[kiln.source.native_semantics] Git OIDs, Pijul change identities, Jujutsu change IDs, commit IDs, backend kinds, operation identities, bookmark states, Radicle repository and actor identities, GitHub delivery and account identities, and native signatures MUST remain typed provider facts with explicit verification status.

#### Scenario: Equal trees have different history
r[kiln.source.native_semantics.scenario.history]
- GIVEN two source snapshots with equal tree identities and different native history identities
- WHEN Kiln compares them
- THEN it MAY record equal content but MUST NOT claim equal history, signatures, review, or trust.

#### Scenario: Jujutsu resolution is ambiguous
r[kiln.source.native_semantics.scenario.jujutsu_ambiguous]
- GIVEN a Jujutsu change ID, bookmark, or revset that resolves to multiple commits
- WHEN immutable source resolution runs
- THEN Kiln MUST reject the observation before acquisition and MUST report the divergent or conflicted native facts.

### Requirement: Jujutsu source provider
r[kiln.source.jujutsu] Kiln MUST provide a versioned Jujutsu source-provider profile and adapter for admitted repository observation, bookmark or revset discovery, exact-one commit resolution, and snapshot acquisition without forge or collaboration authority.

#### Scenario: Supported Jujutsu bookmark resolves exactly
r[kiln.source.jujutsu.scenario.resolve]
- GIVEN an admitted Jujutsu version and backend whose bookmark resolves to one commit
- WHEN the source adapter resolves and acquires that bookmark
- THEN it MUST bind the operation identity, bookmark, change ID, exact commit ID, backend kind, and canonical tree identity.

#### Scenario: Jujutsu backend is unsupported
r[kiln.source.jujutsu.scenario.backend]
- GIVEN a Jujutsu provider profile with an unsupported backend kind
- WHEN provider admission runs
- THEN the provider MUST fail before repository observation or source acquisition.

### Requirement: Exact proposed-change candidate
r[kiln.source.proposed_change] A proposed-change evaluation MUST use an exact candidate snapshot produced under a named provider strategy and MUST record base, head, strategy, and resulting snapshot identities.

#### Scenario: Provider cannot create candidate snapshot
r[kiln.source.proposed_change.scenario.conflict]
- GIVEN a proposed change whose base and head cannot form the requested candidate
- WHEN the provider evaluates the strategy
- THEN Kiln MUST record the bounded conflict or denial and MUST NOT invent a candidate snapshot.

### Requirement: Explicit mirror policy
r[kiln.source.mirrors] Multiple source providers for one logical project MUST use an explicit mirror policy that defines identity comparison, authority, disagreement, and fallback behavior.

#### Scenario: Mirrors disagree on native revision
r[kiln.source.mirrors.scenario.disagreement]
- GIVEN two admitted mirrors that return different native revisions for one requested reference
- WHEN mirror resolution runs
- THEN Kiln MUST apply the declared disagreement policy and MUST NOT silently choose one result.

### Requirement: Trigger security and replay policy
r[kiln.source.trigger_security] Trigger shells MUST verify protocol-required authentication, bind native delivery facts, apply configured replay policy, and pass only bounded observations into the core.

#### Scenario: GitHub signature is invalid
r[kiln.source.trigger_security.scenario.github]
- GIVEN a GitHub webhook with an invalid required HMAC-SHA256 signature
- WHEN trigger intake runs
- THEN the trigger MUST be rejected before a Kiln command is admitted.

### Requirement: Separate discovery, acquisition, and mutation
r[kiln.source.observation_split] Source discovery, exact version resolution, snapshot acquisition, and provider mutation MUST use separate requests, capabilities, observations, and effect authority.

#### Scenario: Webhook requests a source check
r[kiln.source.observation_split.scenario.webhook]
- GIVEN an authenticated webhook that requests immediate source discovery
- WHEN trigger intake runs
- THEN it MAY request a discovery observation but MUST NOT acquire, merge, push, or mutate source without separate admitted effects.

### Requirement: Source adapter fixtures
r[kiln.source.fixtures] Maintained fixtures MUST include positive Git, Pijul, Jujutsu, GitHub, and Radicle mappings and negative stale, missing, unsupported-backend, ambiguous, conflicted, divergent, forged, replayed, mismatched, and disagreement cases.

#### Scenario: Candidate lacks Pijul rejection fixtures
r[kiln.source.fixtures.scenario.missing]
- GIVEN a source adapter candidate without malformed and missing-change fixtures
- WHEN adapter readiness runs
- THEN the candidate MUST fail the source fixture gate.

#### Scenario: Candidate lacks Jujutsu ambiguity fixtures
r[kiln.source.fixtures.scenario.jujutsu_missing]
- GIVEN a Jujutsu adapter candidate without divergent-change and conflicted-bookmark fixtures
- WHEN adapter readiness runs
- THEN the candidate MUST fail the source fixture gate.

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
