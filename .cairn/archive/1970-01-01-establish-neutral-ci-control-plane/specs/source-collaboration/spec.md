# Source and Collaboration Specification

## ADDED Requirements

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
