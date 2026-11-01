# Agent Harness Friction Specification

## Purpose

Defines declared-harness aggregation in review-friction observation.

## ADDED Requirements

### Requirement: Harness aggregation

r[kiln.observe.review.agent.harness] A review-window change MUST carry the
declared harnesses of its attested revisions. The friction view MUST expose a
top-level `agent_harnesses` map counting changes once per declared harness.
Missing harness metadata MUST NOT be synthesized. Empty maps MUST be omitted and
preserve earlier identities; non-empty maps MUST participate in identity.

#### Scenario: Harness count
r[kiln.observe.review.agent.harness.scenario.count]
- GIVEN one attested change with declared harness `omp`
- WHEN friction is projected
- THEN `agent_harnesses["omp"]` is 1.

#### Scenario: No harness metadata
r[kiln.observe.review.agent.harness.scenario.missing]
- GIVEN an attested change without a declared harness
- WHEN friction is projected
- THEN no synthetic harness entry is added.

### Requirement: Declared-only authority boundary

r[kiln.observe.review.agent.harness.boundary] Harness counts MUST remain declared
observation data. They MUST NOT verify execution or alter authorship basis,
review, CI, merge or release authority.

#### Scenario: Observation only
r[kiln.observe.review.agent.harness.boundary.scenario.authority]
- GIVEN a friction view with harness counts
- WHEN a consumer reads it
- THEN the existing non-authoritative basis and non-claims remain unchanged.
