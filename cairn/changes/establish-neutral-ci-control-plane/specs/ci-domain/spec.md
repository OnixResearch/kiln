# CI Domain Specification

## ADDED Requirements

### Requirement: Typed CI domain model
r[kiln.domain.model] Kiln MUST define bounded typed models for projects, pipeline revisions, triggers, evaluations, runs, tasks, attempts, products, metrics, notifications, commands, events, effects, decisions, and diagnostics.

#### Scenario: Valid project starts an evaluation
r[kiln.domain.model.scenario.evaluation]
- GIVEN an admitted project, immutable pipeline revision, and valid trigger
- WHEN the domain core decides the evaluation command
- THEN it MUST emit typed evaluation events and only declared effect requests.

### Requirement: Named domain bounds
r[kiln.domain.bounds] Every collection, identifier, payload reference, diagnostic set, and transition batch MUST have a named admitted bound outside transition code.

#### Scenario: Command exceeds an admitted bound
r[kiln.domain.bounds.scenario.exceeded]
- GIVEN a command with more task declarations than its admitted profile permits
- WHEN command admission runs
- THEN the command MUST be denied without events or effects.

### Requirement: Stable canonical identity
r[kiln.domain.identity] Kiln-owned canonical artifacts MUST use versioned packed Preserves bytes and BLAKE3 identities, while human aliases and provider-native identifiers remain separate typed facts.

#### Scenario: Provider alias changes
r[kiln.domain.identity.scenario.alias]
- GIVEN one canonical run and a changed provider display alias
- WHEN Kiln rebuilds its status projection
- THEN the run identity MUST remain unchanged and the alias MUST remain non-authoritative.

### Requirement: Pure production transition
r[kiln.domain.transition] Kiln MUST use one deterministic `decide` relation for commands and one deterministic `evolve` relation for events without filesystem, process, network, clock, credential, or persistence effects.

#### Scenario: Equal state and command are repeated
r[kiln.domain.transition.scenario.deterministic]
- GIVEN byte-equivalent admitted state and command values
- WHEN the production decision relation runs twice
- THEN both decisions, diagnostics, events, and effects MUST be equal.

### Requirement: Explicit lifecycle invariants
r[kiln.domain.lifecycle] Evaluation, run, task, and attempt states MUST use explicit admitted transitions, and terminal states MUST reject later mutation except through a versioned correction event.

#### Scenario: Completion arrives after terminal failure
r[kiln.domain.lifecycle.scenario.terminal]
- GIVEN an attempt in a terminal failed state
- WHEN a normal success completion command arrives
- THEN the command MUST be denied and the terminal state MUST remain unchanged.

### Requirement: Distinct attempts and unknown outcomes
r[kiln.domain.attempts] Every retry MUST create a distinct attempt identity, and an ambiguous provider observation MUST remain an unknown outcome until an admitted reconciliation or operator decision resolves it.

#### Scenario: Submission connection fails after dispatch
r[kiln.domain.attempts.scenario.unknown]
- GIVEN a provider dispatch that might have reached the provider
- WHEN the shell reports a connection failure without provider proof
- THEN Kiln MUST record an unknown outcome and MUST NOT classify the attempt as not started.

### Requirement: Typed products and metrics
r[kiln.domain.products] Products and metrics MUST be typed records linked to the exact producing attempt and provider observations, and MUST NOT depend on hidden output-directory metadata files.

#### Scenario: Product lacks a producing attempt
r[kiln.domain.products.scenario.orphan]
- GIVEN a product observation without an admitted producing attempt identity
- WHEN the core validates the completion command
- THEN the product MUST be rejected and MUST NOT enter run status.

### Requirement: Positive and negative domain fixtures
r[kiln.domain.fixtures] The maintained domain suite MUST include accepted lifecycle fixtures and rejected malformed, stale, duplicate, oversized, terminal-mutation, and unknown-outcome fixtures.

#### Scenario: Candidate omits negative lifecycle tests
r[kiln.domain.fixtures.scenario.missing_negative]
- GIVEN a release candidate without current terminal-mutation and stale-command fixtures
- WHEN release validation runs
- THEN the candidate MUST fail the domain fixture gate.

### Requirement: Domain claim boundary
r[kiln.domain.boundary] Passing domain checks MUST NOT claim source truth, provider correctness, authorization, sandboxing, build correctness, workflow correctness, external effect success, exactly-once execution, release eligibility, or deployment safety.

#### Scenario: Run reaches success
r[kiln.domain.boundary.scenario.success]
- GIVEN a run with admitted terminal provider observations
- WHEN Kiln reports success
- THEN the report MUST remain limited to the recorded CI transition and observation facts.
