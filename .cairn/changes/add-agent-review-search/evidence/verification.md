# Verification evidence

## Result

Review search supports read-only agent-attestation and declared-harness filters. `agent=with`
matches only changes whose current revision has `submitted_by`; `agent=without`
matches only unattested current revisions; absence does not filter. The console
form and saved views carry the same filter.

## Evidence

- Core search test covers with, without and absent modes.
- Saved-query coverage retains the agent filter.
- Web integration test creates an agent-submitted change and verifies positive
  and negative filters plus malformed-value rejection.
- The complete workspace test suite and workspace-wide Clippy pass for the
agent, harness and model filters.
- Clippy, rustfmt, JavaScript syntax and Cairn validation pass.

## Non-claims

Attestation remains declared principal identity, not verified execution or
authorship. Historical revisions are not backfilled, and filtering changes no
review state or approval authority.
