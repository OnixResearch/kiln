## Context

`SearchQuery` already carries kind, text, status and advisory filters, with
optional fields omitted so saved views keep their previous bytes. Revision
`submitted_by` is the trusted attested-principal fact.

## Decisions

- Add `Option<bool>` `agent` to `SearchQuery`; absence means no filter.
- Evaluate the filter against the current revision only, matching the review
  detail's current identity view.
- Parse only the named `with`/`without` values in the shell; malformed values are
  rejected as invalid queries.
- Add the same filter to the console form and saved-view payloads.
- The filter is read-only; it does not alter attestation or any authority.

## Risks

- Historical revisions without attestation match `without`.
- The filter reflects the configured agent set and attestation facts; it does
  not verify execution or authorship.
