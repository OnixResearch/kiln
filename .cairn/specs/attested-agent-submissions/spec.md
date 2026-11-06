# Attested Agent Submissions Specification

## Purpose

Defines the `attested-agent-submissions` capability.

## Requirements

### Requirement: Trusted agent submission attestation

r[kiln.review.agent.attestation] When a `Create` or `Upload` command is submitted
by a principal in the trusted review-profile agent set, the core MUST record the
exact principal on the revision and MUST omit the field for every other
principal. It MUST reject an agent principal declaring `human` authorship before
mutation with `agent_authorship_mismatch`. Human submissions MUST remain
unchanged, and undeclared or agent-declared authorship MUST remain valid.

#### Scenario: Agent submission is attested
r[kiln.review.agent.attestation.scenario.agent]
- GIVEN a declared agent principal creating a revision
- WHEN the command is applied
- THEN the revision records that principal as `submitted_by`.

#### Scenario: Agent human declaration is rejected
r[kiln.review.agent.attestation.scenario.human]
- GIVEN a declared agent principal declaring human authorship
- WHEN the command is applied
- THEN it fails before mutation and no revision is created.

### Requirement: Attested identity is visibly separate

r[kiln.review.agent.console] The review list, header and identity view MUST show an
attested agent principal separately from self-declared authorship and MUST
state that authorship remains self-declared and unverified. The display MUST add no
authority or merge effect.

#### Scenario: Attested principal label
r[kiln.review.agent.console.scenario.label]
- GIVEN a revision with `submitted_by`
- WHEN the identity view renders
- THEN it labels the principal as attested and keeps the self-declared
  authorship note.
