# Add authenticated GitHub issue polling transport

## Why

`poll_implementation_issues` already bounds and normalizes recovery pages, but
the authenticated HTTP transport and its response/page safeguards were absent.
Issue OnixResearch/kiln#44 required bounded polling recovery alongside webhooks;
this closes that transport gap without adding a scheduler or dispatch authority.

## What Changes

- Add `GithubIssuePollClient` against a fixed API origin with bearer auth, no
  redirects/proxy, bounded response bytes and deadlines.
- Reject invalid request identifiers, filter pull requests from the issues feed,
  validate timestamps/actors/states, and parse one strictly advancing Link page.
- Distinguish unauthorized, not-found, rate-limited, transport and payload
  failures; no retries are added.
- Use the existing bounded DNS child and issue normalizer; document that schedule
  and queue wiring remain separate.

## Impact

- **Files**: `crates/kiln-adapter-github/src/poll_http.rs`, `poll.rs` helper
  visibility, `status.rs` resolver visibility, docs and specification.
- **Testing**: authenticated fixture page, PR filtering, Link pagination,
  refused credentials, malformed payload, non-advancing/oversized Link.
- **Authority**: transport only. No scheduling, admission, dispatch or retry.
