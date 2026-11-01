## Context

Access viewer authentication yields principal ids `viewer:<id>`. Review profile
agent entries are exact write principals used by advisory/binding vote policy
and submission attestation. The server loads access before constructing the
review workspace.

## Decisions

- `agent` is optional on a viewer binding. Empty, untrimmed, control-containing
  or over-bound harness/model text is rejected with the rest of the access
  policy.
- The access-derived set contains only viewers with an agent declaration.
- At service startup the server unions those ids into the review profile's
  `agent_principals` without duplicates. The review profile admission then
  enforces that every agent is an admitted write principal.
- The declared harness/model is not yet copied onto revisions; this change
  derives identity only.
- Existing profiles that omit `agent` keep identical behavior.

## Risks

- An access agent absent from review write principals fails review startup rather
  than silently becoming ineffective.
- Review-profile and access lists can still both contribute entries; the union
  is deterministic.
- The declaration does not prove which model or harness executed a submission.
