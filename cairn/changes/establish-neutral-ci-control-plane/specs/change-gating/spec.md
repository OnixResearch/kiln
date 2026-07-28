# Change Gating Specification

## ADDED Requirements

### Requirement: Provider-neutral candidate sets
r[kiln.gating.candidate_set] Kiln MUST model a gate candidate as an ordered bounded set of exact proposed changes, target revisions, source snapshots, dependency edges, and composition observations without requiring Git, Gerrit, GitHub, or Radicle semantics.

#### Scenario: Candidate spans different collaboration providers
r[kiln.gating.candidate_set.scenario.mixed]
- GIVEN proposed changes from two admitted collaboration providers
- WHEN Kiln forms one cross-project gate candidate
- THEN the candidate MUST preserve each native identity and MUST use exact Kiln source and change references.

### Requirement: Canonical candidate-set identity
r[kiln.gating.identity] A candidate-set identity MUST bind the ordered changes, dependency graph, exact target revisions, exact source snapshots, composition strategy, pipeline revisions, and gate policy with canonical packed Preserves and BLAKE3.

#### Scenario: Earlier dependency changes
r[kiln.gating.identity.scenario.dependency_change]
- GIVEN two candidate sets that differ only in one earlier dependency revision
- WHEN canonical identity is calculated
- THEN the candidate-set identities MUST differ.

### Requirement: Validated dependency graph
r[kiln.gating.dependencies] Candidate dependencies MUST form a bounded graph under a named cycle, missing-node, cross-project, ordering, and provider-compatibility policy.

#### Scenario: Unadmitted cycle appears
r[kiln.gating.dependencies.scenario.cycle]
- GIVEN proposed changes that form a dependency cycle under a policy that rejects cycles
- WHEN candidate admission runs
- THEN the candidate MUST be denied without evaluation or merge effects.

### Requirement: Frozen source universe
r[kiln.gating.frozen_sources] Every evaluation in one candidate set MUST use the same admitted exact revision for each referenced repository and MUST NOT resolve a moving reference after candidate admission.

#### Scenario: Unrelated branch moves during evaluation
r[kiln.gating.frozen_sources.scenario.branch_move]
- GIVEN an admitted candidate with a frozen repository state
- WHEN a provider branch moves before a later task starts
- THEN the task MUST retain the candidate's exact repository revision.

### Requirement: Explicit speculative evaluation
r[kiln.gating.speculation] Kiln MAY evaluate dependent candidate prefixes in parallel only when the gate policy declares speculative execution, resource bounds, candidate ordering, and result scope.

#### Scenario: Speculation is disabled
r[kiln.gating.speculation.scenario.disabled]
- GIVEN a serial gate policy
- WHEN multiple changes become eligible
- THEN Kiln MUST NOT start a dependent candidate before every required earlier gate result is accepted.

### Requirement: Deterministic invalidation and reevaluation
r[kiln.gating.invalidation] When an earlier proposed change fails, changes, or leaves the queue, every result whose candidate identity depended on it MUST become inapplicable, and any continued change MUST receive a new candidate identity and evaluation.

#### Scenario: First candidate fails
r[kiln.gating.invalidation.scenario.first_fails]
- GIVEN evaluations for candidate prefixes containing changes A and B
- WHEN A fails and B remains eligible
- THEN B's result that included A MUST NOT authorize merge and Kiln MUST form a new candidate without A.

### Requirement: Named gate window policy
r[kiln.gating.window] Gate concurrency and speculative windows MUST use named bounds and update rules with explicit floors, ceilings, increase, decrease, fairness interaction, and disabled behavior.

#### Scenario: Failure shrinks a configured adaptive window
r[kiln.gating.window.scenario.shrink]
- GIVEN an adaptive gate policy and a failed head candidate
- WHEN the pure window transition runs
- THEN it MUST apply the configured decrease rule without crossing the configured floor.

### Requirement: Merge preflight and authority separation
r[kiln.gating.merge_authority] A successful gate result MUST request a collaboration-provider merge effect only after a fresh preflight proves the expected target, candidate, approvals, provider capability, and policy facts still match.

#### Scenario: Target moved after successful tests
r[kiln.gating.merge_authority.scenario.target_moved]
- GIVEN a successful candidate evaluation and a changed target revision
- WHEN merge preflight runs
- THEN Kiln MUST deny the merge effect and MUST require a newly composed candidate.

### Requirement: Change-gating fixtures
r[kiln.gating.fixtures] Maintained gating tests MUST include serial and speculative success, cross-project dependencies, stable ordering, invalidation, reevaluation, and merge preflight plus malformed, missing, cyclic, stale, oversized, moving-target, and provider-mismatch cases.

#### Scenario: Invalidation fixture is absent
r[kiln.gating.fixtures.scenario.missing]
- GIVEN a release candidate without a current failed-prefix invalidation fixture
- WHEN gating readiness runs
- THEN the candidate MUST fail the gating fixture gate.

### Requirement: Gating non-claims
r[kiln.gating.nonclaims] Passing a gate MUST NOT claim source trust, test completeness, provider correctness, merge success, future compatibility, or correctness outside the exact candidate and policy.

#### Scenario: Candidate passes every configured task
r[kiln.gating.nonclaims.scenario.passed]
- GIVEN a candidate with accepted terminal task results
- WHEN Kiln reports gate success
- THEN the report MUST remain scoped to that candidate, pipeline revision, provider observations, and gate policy.
