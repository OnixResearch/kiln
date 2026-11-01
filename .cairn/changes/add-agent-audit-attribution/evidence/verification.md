# Verification evidence

## Result

Attributable audit records now carry the authenticated principal's declared agent
identity when present. The denied-viewer test confirms the declared harness is
recorded. Non-agent and anonymous records remain unchanged and no authority
decision changes.

## Evidence

- The focused denied-viewer test asserts agent attribution in the activity
  audit.
- Full core and web-service suites pass.
- Clippy, rustfmt and Cairn validation pass.

## Non-claims

The declaration is configuration, not verified execution metadata. Historical
records are not backfilled, and signed per-request agent credentials remain
future work.
