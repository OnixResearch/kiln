## Why

Authenticated webhook intake covers low-latency delivery, but a missed or
unreachable delivery needs bounded polling recovery. Polling must produce the
same work contract and never become an authority bypass.

Issue: [#44](https://github.com/OnixResearch/kiln/issues/44).

## What Changes

- Add `IssuePollTransport`, a provider port that owns authenticated bounded HTTP.
- Add `poll_implementation_issues` with reviewed page and record bounds.
- Normalize polled records through the same operator policy and work contract as
  webhook intake, with a content-bound source identity.
- Keep the highest update sequence for duplicate records across pages.
- Sort observations deterministically by update sequence and issue number.
- Fail closed when more records remain beyond the page bound.

## Impact

- **Files**: `crates/kiln-adapter-github/src/poll.rs`,
  `crates/kiln-adapter-github/src/implementation.rs`,
  `docs/implementer-service.md`.
- **Testing**: two-page normalization, duplicate sequence selection, unenrolled
  actor and page bound refusal, stable content-bound identity.
- **Authority**: recovery only. No HTTP client, schedule, persistence or
  dispatch is added; results still pass ledger admission and claims.
