## Context

Access viewer declarations contain bounded harness/model text. Review profile
agent principals drive advisory/binding votes and submission attestation.
Revision `submitted_by` originally stored only the principal string.

## Decisions

- `AgentIdentity` is a trusted fact. Review profile metadata is admitted only
  for declared agents with bounded non-control text.
- `AttestedSubmission` stores the exact principal and optional identity as one
  object, omitted for human and unattested revisions.
- Access declarations merge through one profile method that deduplicates and
  rejects conflicting explicit metadata. Review write-principal admission still
  runs afterward.
- The Nickel review profile exports optional `agent_principals` and
  `agent_metadata`; previously they were accepted but omitted from the output.
- The browser renders the declared harness/model beside the attested principal
  and keeps the self-declared authorship warning.
- No metadata is used for authority; votes and evidence gates are unchanged.

## Risks

- Metadata is operator-declared and can be wrong; it is not execution proof.
- Profiles with access and review metadata for the same principal must agree or
  startup fails.
- Historical revisions remain unattested and are not backfilled.
