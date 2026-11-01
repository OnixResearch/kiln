# Verification evidence

## Result

Review search supports a read-only agent-attestation filter. `agent=with`
matches only changes whose current revision has `submitted_by`; `agent=without`
matches only unattested current revisions; absence does not filter. The console
form and saved views carry the same filter.

## Evidence

- Core search test covers with, without and absent modes.
- Web integration test verifies shell parsing and returned change counts.
- Full core and web-service suites pass.
- Clippy, rustfmt, JavaScript syntax and Cairn validation pass.

## Non-claims

Attestation remains declared principal identity, not verified execution or
authorship. Historical revisions are not backfilled, and filtering changes no
review state or approval authority.
