# Git Source Adapter Specification

## Purpose

Defines the `git-source-adapter` capability.

## Requirements

### Requirement: Typed read-only Git profile
r[kiln.git.profile] Kiln MUST admit Git through a typed profile with exact library, format, feature, capability, platform, bound, and non-claim facts.

#### Scenario: Local profile matches
r[kiln.git.profile.scenario.match]
- GIVEN a complete profile for the pinned local Git cohort
- WHEN source-provider admission runs
- THEN Kiln MAY make discovery, exact resolution, acquisition, and identity observation available.

#### Scenario: Repository feature is unsupported
r[kiln.git.profile.scenario.feature]
- GIVEN a repository that requires an undeclared extension or object format
- WHEN repository admission runs
- THEN Kiln MUST reject it before object traversal.

### Requirement: Exact Git resolution
r[kiln.git.resolution] A Git resolution MUST preserve repository, symbolic reference, exact commit, object format, native tree, and provider observation identities.

#### Scenario: Reference is stable
r[kiln.git.resolution.scenario.stable]
- GIVEN a selected reference that remains stable during resolution and acquisition
- WHEN the adapter resolves it
- THEN it MUST return one exact commit and its native tree facts.

#### Scenario: Reference moves
r[kiln.git.resolution.scenario.moved]
- GIVEN a selected reference that moves during acquisition
- WHEN consistency admission runs
- THEN Kiln MUST reject the mixed observation.

### Requirement: Canonical Git snapshot
r[kiln.git.snapshot] The adapter MUST compute a canonical BLAKE3 tree identity from bounded entries and bytes for the exact admitted Git commit.

#### Scenario: Snapshot is valid
r[kiln.git.snapshot.scenario.valid]
- GIVEN a stable commit with supported bounded entries
- WHEN snapshot admission and framing run
- THEN the adapter MUST return one deterministic canonical tree identity.

#### Scenario: Snapshot has an unsafe path
r[kiln.git.snapshot.scenario.path]
- GIVEN a tree with an unsafe path, duplicate entry, unsupported kind, or exceeded bound
- WHEN snapshot admission runs
- THEN Kiln MUST reject it without a canonical tree identity.

### Requirement: Read-only Git capabilities
r[kiln.git.capabilities] The first Git profile MUST exclude network, credential, mutation, signing, and collaboration capabilities.

#### Scenario: Local acquisition is requested
r[kiln.git.capabilities.scenario.read]
- GIVEN an admitted local repository and exact revision request
- WHEN capability admission runs
- THEN Kiln MAY admit bounded read access.

#### Scenario: Fetch is requested
r[kiln.git.capabilities.scenario.fetch]
- GIVEN the first read-only profile
- WHEN a request attempts fetch, push, merge, signing, or branch mutation
- THEN Kiln MUST reject it before external effects.

### Requirement: Git adapter conformance
r[kiln.git.conformance] Maintained conformance MUST include real repositories produced or consumed by the pinned Git cohort.

#### Scenario: Only handwritten objects exist
r[kiln.git.conformance.scenario.native]
- GIVEN an adapter candidate without real repository fixtures
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: Git adapter non-claims
r[kiln.git.nonclaims] Passing checks MUST NOT claim source trust, signature trust, complete history, remote availability, or forge state.

#### Scenario: Local snapshot passes
r[kiln.git.nonclaims.scenario.scope]
- GIVEN a passing local snapshot
- WHEN Kiln records evidence
- THEN it MUST remain scoped to the checked repository, objects, bytes, profile, bounds, and observations.
