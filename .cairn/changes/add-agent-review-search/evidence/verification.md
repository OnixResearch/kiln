# Verification evidence

## Result

Review search supports read-only agent-attestation, declared-harness and declared-model filters. `agent=with`
matches only changes whose current revision has `submitted_by`; `agent=without`
matches only unattested current revisions; absence does not filter. The console
form and saved views carry the same filters.

## Evidence

- Core search test covers with, without and absent modes.
- Web coverage verifies empty harness and model values do not filter.
- Saved-query coverage retains the agent filter.
- Web integration test creates an agent-submitted change, verifies positive and
  negative filters, malformed-value rejection and a saved-view round trip for
  harness and model.
- The complete workspace test suite and workspace-wide Clippy pass for the
  agent, harness and model filters.
- Live-console tests pass, including the saved-query builder preserving every
  agent filter, omitting absent fields and normalizing route text to the same
  trimmed UTF-8 byte bound.
- Clippy, rustfmt, JavaScript syntax and Cairn validation pass.

## Non-claims

Attestation remains declared principal identity, not verified execution or
authorship. Historical revisions are not backfilled, and filtering changes no
review state or approval authority.
