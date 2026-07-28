# Repository Hosting Specification

## ADDED Requirements

### Requirement: Canonical Radicle source governance
r[kiln.hosting.canonical_radicle] Kiln MUST plan Radicle as its canonical source and collaboration host, while treating other Git endpoints as declared acquisition or mirror projections.

#### Scenario: GitHub mirror has a different branch tip
r[kiln.hosting.canonical_radicle.scenario.mirror_tip]
- GIVEN a Radicle canonical revision and a GitHub mirror with another branch tip
- WHEN repository authority is evaluated
- THEN the Radicle governed signed refs MUST remain canonical and the mirror drift MUST be reported.

### Requirement: Runtime independence from repository hosting
r[kiln.hosting.runtime_independence] Kiln core, standalone builds, source archives, and local tests MUST work without a Radicle node, `rad` command, Radicle identity, or network connection.

#### Scenario: Radicle software is absent
r[kiln.hosting.runtime_independence.scenario.absent]
- GIVEN an exact Kiln source archive and no Radicle software
- WHEN the standalone build and core tests run
- THEN they MUST work without a Radicle runtime dependency.

### Requirement: Typed hosting profile
r[kiln.hosting.profile] Human-authored repository hosting policy MUST use a typed Nickel profile that records intended and observed hosting state, canonical branch, RID state, delegates, threshold, signed-ref feature policy, seed set, required replica observations, acquisition routes, mirrors, license, rollback, and non-claims.

#### Scenario: Published profile has no RID
r[kiln.hosting.profile.scenario.missing_rid]
- GIVEN a hosting profile marked as published
- WHEN profile admission runs without an assigned RID
- THEN admission MUST fail before publication evidence is accepted.

### Requirement: Explicit publication authority
r[kiln.hosting.publication_authority] Radicle initialization, signing, push, synchronization, announcement, delegate change, threshold change, and mirror mutation MUST require an explicit operator action outside pure validation and release planning.

#### Scenario: Release validation runs
r[kiln.hosting.publication_authority.scenario.validation]
- GIVEN a valid local publication candidate without operator publication authority
- WHEN release validation runs
- THEN it MUST produce no Radicle signing, push, synchronization, announcement, or remote mutation.

### Requirement: Exact immutable publication candidate
r[kiln.hosting.candidate] A publication candidate MUST bind the exact source revision, canonical archive BLAKE3 identity, license, lifecycle state, release evidence, rollback revision, and hosting profile before remote mutation.

#### Scenario: Source changes after candidate validation
r[kiln.hosting.candidate.scenario.drift]
- GIVEN a validated local candidate and changed source bytes
- WHEN publication preflight runs
- THEN it MUST reject the stale candidate before any Radicle effect.

### Requirement: Governed signed references
r[kiln.hosting.signed_refs] Publication evidence MUST bind the assigned RID, exact source revision, configured delegates, configured threshold, and required signed-reference feature profile.

#### Scenario: Signed-reference threshold differs
r[kiln.hosting.signed_refs.scenario.threshold]
- GIVEN observed Radicle signed refs whose threshold differs from the admitted hosting profile
- WHEN publication evidence is validated
- THEN validation MUST fail without treating the source as canonically published.

### Requirement: Independent acquisition evidence
r[kiln.hosting.acquisition] Published source MUST be acquired through the configured native Radicle and read-only routes, and every required observation MUST resolve the exact revision and canonical archive identity.

#### Scenario: Required replica observation is missing
r[kiln.hosting.acquisition.scenario.replica]
- GIVEN a hosting profile that requires a named set or count of replica observations
- WHEN publication evidence lacks one required observation
- THEN publication readiness MUST fail without claiming distributed availability.

### Requirement: Non-authoritative mirrors
r[kiln.hosting.mirrors] GitHub, HTTPS, and other mirrors MUST declare direction, mutability, synchronization policy, and authority, and MUST NOT silently replace Radicle governance.

#### Scenario: Read-only mirror accepts a write
r[kiln.hosting.mirrors.scenario.write]
- GIVEN a mirror profile declared as read-only
- WHEN validation observes a successful write route
- THEN mirror admission MUST fail and the route MUST NOT count as a governed acquisition path.

### Requirement: Non-circular bootstrap
r[kiln.hosting.bootstrap] Kiln repository publication and recovery MUST NOT require a running Kiln service or passing Kiln-hosted CI result.

#### Scenario: Kiln service is unavailable
r[kiln.hosting.bootstrap.scenario.unavailable]
- GIVEN a valid local candidate and unavailable Kiln service
- WHEN an authorized operator performs repository recovery
- THEN recovery MAY use Radicle and verified source evidence without depending on Kiln runtime state.

### Requirement: Explicit hosting recovery
r[kiln.hosting.recovery] Hosting policy MUST define source backup, signed-reference capture, seed loss, mirror drift, delegate unavailability, rollback, and governance-change procedures without automatic authority migration.

#### Scenario: Every configured seed is unavailable
r[kiln.hosting.recovery.scenario.seed_loss]
- GIVEN retained canonical source and signed-reference evidence but unavailable configured seeds
- WHEN recovery is planned
- THEN the system MUST report unavailable distribution and MUST require explicit governed recovery action.

### Requirement: Consistent publication bundle
r[kiln.hosting.bundle_consistency] One publication receipt MUST bind the hosting profile, candidate, RID, governed signed refs, source revision, archive identity, license, lifecycle evidence, replica observations, and acquisition results as one versioned set.

#### Scenario: Receipt mixes two publication observations
r[kiln.hosting.bundle_consistency.scenario.mix]
- GIVEN signed refs from one publication and an archive observation from another
- WHEN publication validation runs
- THEN it MUST reject the mixed bundle without claiming canonical publication.

### Requirement: Freshness and rollback policy
r[kiln.hosting.freshness_rollback] Hosting policy MUST define monotonic publication sequence, observation freshness, stale-data, rollback, freeze, and mirror-disagreement behavior over explicit shell-supplied time facts.

#### Scenario: Mirror serves an older valid publication
r[kiln.hosting.freshness_rollback.scenario.older]
- GIVEN a client state that admitted a newer publication sequence
- WHEN a mirror serves an older otherwise valid source bundle
- THEN validation MUST report rollback or stale data and MUST NOT accept it as the current publication.

### Requirement: Hosting fixture matrix
r[kiln.hosting.fixtures] Maintained hosting tests MUST include valid planned and published profiles plus invalid RID, candidate, license, delegate, threshold, signed-ref, replica, mirror, write-route, bootstrap, and recovery cases.

#### Scenario: Candidate lacks mirror-drift fixtures
r[kiln.hosting.fixtures.scenario.missing]
- GIVEN a release candidate without current mirror-drift and write-route rejection fixtures
- WHEN hosting readiness runs
- THEN the candidate MUST fail the hosting fixture gate.

### Requirement: Hosting non-claims
r[kiln.hosting.nonclaims] Passing hosting checks MUST NOT claim future availability, universal write exclusion, delegate honesty, Radicle correctness, Git correctness, mirror freshness, source correctness, CI correctness, or release eligibility.

#### Scenario: Every configured route agrees
r[kiln.hosting.nonclaims.scenario.scope]
- GIVEN matching native and read-only acquisition observations
- WHEN publication evidence is reported
- THEN the report MUST remain limited to the exact observed RID, refs, revision, archive, routes, and policy.
