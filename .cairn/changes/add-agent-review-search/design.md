## Context

`SearchQuery` already carries kind, text, status and advisory filters, with
optional fields omitted so saved views keep their previous bytes. Revision
`submitted_by` is the trusted attested-principal fact.

## Decisions

- Add `Option<bool>` `agent` and optional `agent_harness` / `agent_model` to
  `SearchQuery`; absence means no filter.
- Evaluate every filter against the current revision only, matching the review
  detail's current identity view.
- Harness and model match only the exact declared value. A revision without an
  attested agent or declared model does not match a model filter.
- Parse only the named `with`/`without` values for `agent`; parse harness and
  model as trimmed, control-free values of at most 128 bytes. Reject every other
  form as an invalid query.
- Add the same filters to the console form, URL state and saved-view payloads;
  saving, applying and clearing a view preserve all three filters.
- Omit absent fields from serialized queries so older saved-view bytes remain
  unchanged.
- The filters are read-only; they alter no attestation, review state, execution
  or merge authority.

## Risks

- Historical revisions without attestation match `without`.
- A saved harness or model filter can stop matching after agent metadata
  changes; the console keeps the active value visible so the filter does not
  silently appear to reset.
- The filters reflect declared identity and attestation facts; they do not
  verify execution, model use or authorship.
