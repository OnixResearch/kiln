## Decisions

Claim uses the same `claim_next` ranking and lease rules as the library. The
printed JSON is the persisted attempt, so an external worker can carry the exact
identity and fence back for cancel or later results.

Cancel requires the exact identity and fence; there is no force-cancel by work
key. This keeps a stale operator from revoking a replacement attempt.

The default one-hour lease is bounded by the core maximum; `--lease-ms` allows a
shorter reviewed lease.

## Non-claims

A claim is not execution. The tool does not dispatch a workflow, inspect a
candidate, verify evidence, publish, or complete work.
