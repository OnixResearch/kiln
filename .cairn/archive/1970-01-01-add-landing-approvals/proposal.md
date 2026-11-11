# Add independent landing approvals

## Why

Issue #48 requires stale policy/approvals to block a landing observation. The
read-only observer checked exact head, base and required checks but not review
authority.

## What Changes

- Add a bounded required-approval count to landing plans.
- Read one bounded review page and use the latest state per actor.
- Refuse changes requested and count only distinct non-author approvals.
- Extend docs and fixture tests.

## Impact

- **Files**: `crates/kiln-adapter-github/src/landing.rs`, docs and specification.
- **Testing**: exact approvals, missing approvals, author exclusion and changes-requested refusal.
- **Authority**: read-only evidence only; no approval or merge mutation.
