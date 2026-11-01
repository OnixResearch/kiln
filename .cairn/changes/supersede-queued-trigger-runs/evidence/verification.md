# Verification evidence

## Result

Triggers may opt into `supersede_queued`. A successful dispatch cancels older
queued runs for the same project and requested reference when their exact source
revision differs. Running and awaiting-approval runs are untouched, and no
provider control effect is issued.

## Evidence

- A three-push automation scenario leaves one active run and two queued runs;
  the older queued run becomes `Cancelled`, the newest remains queued, and the
  backend dispatch count stays at one.
- The complete web-service test suite passes.
- Package Clippy, rustfmt and the Nickel format check pass.

## Non-claims

This is queued-only supersession. It does not implement running-run
cancellation, provider cancellation semantics, or guarantee that a race with the
queue never leaves a stale completed run. Passing tests do not prove execution
correctness or release readiness.
