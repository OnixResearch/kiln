## Context

`GithubLandingClient::land` refuses unattended landing, and `recover_landed` admits only an exact two-parent merge with matching tree, landing evidence and base read-back. `GithubSourceStatusClient::close` observes the exact issue and returns `closed` only when it is already closed; an open issue is refused with `github_source_conditional_update_unqualified`. The accepted specifications predate both removals.

## Goals and Success Contract

Each modified requirement states what the client does today, every scenario names behaviour a test exercises, and the stale scenario IDs leave the accepted specifications instead of staying as untestable obligations.

## Decisions

### Decision: modify in place and replace stale scenarios with new IDs

**Choice:** keep each requirement's ID and title so existing implementation and verification markers remain valid. Replace `merge.scenario.landed` with `merge.scenario.recovered`, and `source_status.scenario.close` with `source_status.scenario.open`. Do not give the old IDs a new meaning.

**Rationale:** a scenario ID that once meant "the merge request succeeds" must not later mean "an external merge is recovered". New IDs keep history readable, and the requirement IDs still anchor the clients.

### Decision: verify, do not relabel

**Choice:** the recovered scenario is marked on the existing test that recovers an external exact merge with GET requests only. The open scenario is marked on the existing test that refuses an open issue after one GET request. The upload scenario uses the marker its test already carries.

## Risks / Trade-offs

- Reviewers searching for the old scenario IDs will find them only in history; the proposal names both.

## Verification Strategy

Run the GitHub adapter's library tests and Clippy, then the Cairn gates, sync, validation and the active-development traceability profile.
