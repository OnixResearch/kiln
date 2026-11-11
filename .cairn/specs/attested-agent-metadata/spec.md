# Attested Agent Metadata Specification

## Purpose

Defines the `attested-agent-metadata` capability.

## Requirements

### Requirement: Trusted bounded agent metadata

r[kiln.review.agent.metadata] A review profile MAY declare `agent_metadata` for
principals in its agent list. Keys MUST name declared agents; harness and
optional model MUST be bounded, trimmed, non-control text. Access agent declarations MUST
merge by exact principal with duplicate suppression; conflicting explicit
metadata MUST fail startup. A Create or Upload by a declared agent MUST record an
`AttestedSubmission` containing the exact principal and any declared identity.
A successful submission receipt MUST likewise expose the declared identity when
the submitting principal is an agent.
Human and unattested revisions MUST omit the field, preserving old bytes. A
legacy bare-principal string in an existing journal MUST deserialize as a
principal-only `AttestedSubmission`.

#### Scenario: Declared metadata is attested
r[kiln.review.agent.metadata.scenario.attest]
- GIVEN a declared agent with harness/model metadata
- WHEN it creates a revision
- THEN the revision records its exact principal and declared identity.

#### Scenario: Metadata cannot name a non-agent
r[kiln.review.agent.metadata.scenario.invalid]
- GIVEN profile metadata for a principal outside the agent list
- WHEN the profile is admitted
- THEN it MUST be rejected.

#### Scenario: Conflicting access and review metadata
r[kiln.review.agent.metadata.scenario.conflict]
- GIVEN access and review declarations that disagree for one principal
- WHEN the review workspace starts
- THEN startup MUST fail.

### Requirement: Attested metadata display

r[kiln.review.agent.metadata.surface] The review identity view MUST show the
attested principal and declared harness/model separately from self-declared
authorship and MUST retain the unverified-authorship warning. It MUST add no
authority or merge effect.

#### Scenario: Display declared identity
r[kiln.review.agent.metadata.surface.scenario.display]
- GIVEN a revision with attested agent metadata
- WHEN the identity view renders
- THEN the declared harness/model are labeled as attested configuration.
