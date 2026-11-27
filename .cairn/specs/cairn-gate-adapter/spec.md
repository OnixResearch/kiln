# Cairn Gate Adapter Specification

## Purpose

Defines the `cairn-gate-adapter` capability.

## Requirements

### Requirement: Versioned Cairn gate profile
r[kiln.cairn_gate.profile] Kiln MUST admit Cairn through a typed profile with exact executable, contract, policy, capability, platform, and bound facts.

#### Scenario: Profile matches
r[kiln.cairn_gate.profile.scenario.match]
- GIVEN a complete profile for the pinned Cairn cohort
- WHEN gate-provider admission runs
- THEN Kiln MAY make the declared gate capability available.

#### Scenario: Executable identity differs
r[kiln.cairn_gate.profile.scenario.executable]
- GIVEN a Cairn executable whose BLAKE3 identity differs from the profile
- WHEN shell preflight runs
- THEN Kiln MUST reject the provider before execution.

### Requirement: Exact Cairn gate request
r[kiln.cairn_gate.request] Every Cairn gate request MUST bind the exact candidate, pipeline revision, policy, evidence set, provider profile, and effect identity.

#### Scenario: Request is complete
r[kiln.cairn_gate.request.scenario.valid]
- GIVEN one admitted candidate and policy
- WHEN pure request planning runs
- THEN it MUST produce one deterministic bounded gate request.

#### Scenario: Evidence set changes
r[kiln.cairn_gate.request.scenario.evidence]
- GIVEN a planned request whose required evidence set changes
- WHEN request admission runs
- THEN Kiln MUST require a new request identity.

### Requirement: Exact Cairn result admission
r[kiln.cairn_gate.result] A Cairn result MUST bind the exact request, candidate, policy, evidence set, tool cohort, findings, verdict, and receipt identity.

#### Scenario: Passing result matches
r[kiln.cairn_gate.result.scenario.pass]
- GIVEN a passing result whose applicability facts match the request
- WHEN result admission runs
- THEN Kiln MAY satisfy the corresponding configured gate condition.

#### Scenario: Target moved
r[kiln.cairn_gate.result.scenario.stale]
- GIVEN a passing result for an earlier target revision
- WHEN result admission runs for the current candidate
- THEN Kiln MUST reject the stale result.

### Requirement: Cairn gate reconciliation
r[kiln.cairn_gate.reconciliation] Ambiguous gate execution MUST remain unknown until an exact receipt or admitted deterministic rerun resolves it.

#### Scenario: Process termination is ambiguous
r[kiln.cairn_gate.reconciliation.scenario.unknown]
- GIVEN a gate process whose terminal result was not captured
- WHEN outcome classification runs
- THEN Kiln MUST record unknown without inventing pass or block.

#### Scenario: Rerun uses changed policy
r[kiln.cairn_gate.reconciliation.scenario.policy]
- GIVEN a rerun whose policy identity changed
- WHEN reconciliation admission runs
- THEN Kiln MUST treat it as a new gate request.

### Requirement: Cairn gate conformance
r[kiln.cairn_gate.conformance] Maintained conformance MUST include pinned Cairn producer fixtures and independent positive and negative consumer cases.

#### Scenario: Stale candidate fixture is absent
r[kiln.cairn_gate.conformance.scenario.stale]
- GIVEN an adapter candidate without stale-candidate rejection evidence
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: Cairn gate non-claims
r[kiln.cairn_gate.nonclaims] Passing gate checks MUST NOT claim source correctness, test completeness, release execution, deployment success, or future applicability.

#### Scenario: Gate passes
r[kiln.cairn_gate.nonclaims.scenario.scope]
- GIVEN a result that satisfies one gate condition
- WHEN Kiln records evidence
- THEN it MUST remain scoped to the exact candidate, policy, evidence, tool, and receipt.

### Requirement: Unified Gate Passport and guarded publication/integration recovery surface

r[kiln.cairn.gate_passport_surface]
Runs, Code Reviews, and read-only `kiln-dashboard` reports MUST render a unified **Gate Passport** uniting typed CI checks, `cairn-authoritative-completion` receipts, Patchgate review decisions, SLSA/in-toto subject and dependency digests, Plan Review receipts, and Semantic Merge O/A/B/M quartet observations. Every passport item MUST carry an explicit provenance badge distinguishing `Observed Receipt` (solid border), `Agent Claim` (dotted border), and `Declared` (dashed border), and MUST flag `historical/stale` entries when bound to a superseded candidate. `2.1 Delivery & Cairn` MUST delegate publication and integration recovery actions (`prepare-publication`, `publish`, `recover-integration`, `close-source`) to their owning commands behind Tier 2/3 confirmation rails and fail closed on mismatched candidates or missing operator authority.

#### Scenario: Agent handoff is never rendered as independent canonical verification

r[kiln.cairn.gate_passport_surface.scenario.provenance_separation]
- GIVEN a Cairn unit with an agent self-reported handoff and no independent `cairn-authoritative-completion` receipt
- WHEN the operator opens its Gate Passport in the console or `kiln-dashboard`
- THEN the agent handoff MUST carry the dotted `Agent Claim` badge and the canonical completion gate MUST read as unverified/pending rather than passed
