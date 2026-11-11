## Context

`Facts` is trusted shell input. Review profile already declares exact read and
write principals and the effective policy. Votes are keyed by principal ID and
eligibility already requires a requested eligible non-author reviewer.

## Decisions

- Agent identity is an explicit review-profile fact, not inferred from labels,
  names or content. It is a bounded subset of admitted write principals.
- `Facts.agents` carries those exact principal IDs into the pure checklist.
- `agent_votes` defaults to advisory and is omitted when default, preserving
  existing policy bytes and behavior.
- Advisory agent approvals remain in `Change.votes` so the UI and audit trail
  retain them, but the approval count and owner item exclude them.
- Binding mode is a policy-wide opt-in that removes that exclusion.
- Veto and needs-revision evaluation remains over every eligible vote, so an
  agent cannot bypass an existing blocking signal.
- Missing or duplicate agent profile entries are rejected before service start.

## Risks

- This does not yet prove an agent submitted a change; authorship stays
  self-declared and unattested.
- An operator must keep `agent_principals` synchronized with access identities.
- Binding agent votes are an explicit policy choice; advisory remains the safe
  default.
