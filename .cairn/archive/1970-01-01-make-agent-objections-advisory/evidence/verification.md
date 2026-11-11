# Verification evidence

## Result

Under the default advisory policy, agent vetoes and agent-opened blocking
threads no longer affect the `vetoes` or `threads` checklist items or readiness.
They are retained in a separate non-binding `Checklist.advisory` list and shown
separately in the submit panel. Explicit `agent_votes = binding` keeps the
previous blocking behavior.

## Evidence

- All 185 core library tests pass, including the updated advisory agent test.
- All 167 web-service library tests pass (1 ignored).
- All 26 code-review integration tests pass.
- The code-review JavaScript parses with `node --check`.
- Formatting checks pass for the changed crates.
- `cairn validate --root .` reports zero issues.

## Non-claims

Typed check evidence and agent comment outcome rates from issue #35 remain
unimplemented. Advisory objections are display evidence only and grant no
submission, merge, gate, or release authority. Agent identity comes from
trusted shell facts and is not independently authenticated here.
