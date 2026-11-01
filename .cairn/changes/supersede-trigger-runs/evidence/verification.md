# Verification evidence

## Result

Triggers may opt into `supersede_queued`. A successful dispatch cancels older
queued and active runs for the same project and requested reference when their
exact source revision differs. Queued runs use durable queue control; active
runs use one exact provider cancellation. Awaiting-approval and terminal runs are
untouched.

## Evidence

- A manual run on the same project/reference is not cancelled by a trigger
  dispatch.
- A two-pull-request scenario on the same base leaves the first pull request's
  active run untouched.
- A superseded run exposes `error_code = "superseded"`; the queued control rejects
  an unknown reason without mutation.
- A three-push automation scenario leaves one active run and two queued runs;
  the older active and queued runs become `Cancelled`, the newest remains
  queued, one provider cancellation is issued, and two dispatches occur.
- The complete web-service test suite passes.
- Package Clippy, rustfmt and the Nickel format check pass.

## Non-claims

Supersession is best-effort. It does not guarantee that a race with provider or
queue transitions never leaves a stale completed run. Passing tests do not prove
provider execution correctness or release readiness.
