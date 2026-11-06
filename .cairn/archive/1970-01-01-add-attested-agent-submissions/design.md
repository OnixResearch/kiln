## Context

`Revision` already carries the self-declared `Authorship` and the authenticated
actor is required to equal `revision.author`. Review-profile `agent_principals`
is trusted shell input and is already used to classify votes.

## Decisions

- The core sets `submitted_by` for `Create` and `Upload` from trusted agent facts;
  a request body cannot assert it.
- An agent principal declaring `human` authorship is rejected with
  `agent_authorship_mismatch` before state mutation. Other declared classes stay
  available; undeclared remains valid.
- Human principals and unattested revisions omit the field, preserving old
  journal bytes and behavior.
- The browser labels the attested principal as identity evidence, not verified
  authorship and not authority.
- No harness or model is stored here because this first cut has principal IDs
  only.

## Risks

- Authorship remains self-declared; `submitted_by` attests the configured
  principal, not which model or harness executed the work.
- A review profile agent list is operator-maintained and must stay synchronized
  with access identities.
- Historical revisions have no attestation and are not backfilled.
