# Verification evidence

## Result

Hosted automation can bind an exact tag reference for push triggers. A signed
tag push resolves the admitted tag commit and dispatches an ordinary CI run.
Namespace-specific filter admission rejects branch filters on tag-bound pushes,
tag filters on branch/pull-request triggers, and tag-bound pull requests.
Schedules remain branch-bound.

## Evidence

- End-to-end signed annotated-tag push dispatch passes through real local Git
  tag creation, signature admission, source resolution and dispatch.
- Admission tests reject a tag reference with branch filters, a tag filter that
  does not admit the bound tag, a tag-bound pull-request trigger and a tag-bound
  schedule.
- The complete workspace test suite passes.
- The Nickel contract exports a tag-bound trigger with a tags filter.
- Clippy, rustfmt, Nickel typecheck and Cairn validation pass.

## Non-claims

A tag-bound run is not release evidence and does not publish, move or trust tags.
Tag history is not used as a CI base. Passing tests do not prove provider
execution correctness or release readiness.
