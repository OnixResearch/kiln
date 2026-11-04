## Context

The existing planner computes the canonical candidate, policy, native-binding
and idempotency identities. The reviewer admits the native report and preserves
its exact bytes in `kiln.patchgate.review.v1`. Both operations already use the
same `PatchgateAdapter` and bounded execution path.

The missing production step is receipt publication: `review` writes to stdout,
so an operator or worker must redirect it. A redirect can leave a partial file
on interruption and cannot reject a symlink destination.

## Decisions

### One command for the reviewed run

Add `run` rather than changing `review` output semantics. Existing
plan/review/reconcile consumers remain byte-compatible. `run` loads the intent,
reuses the planner and reviewer helpers, computes the request identity once, and
publishes one receipt.

### Atomic publication

Write the rendered receipt to a private temporary file in the destination
directory, synchronize the file, rename it over the destination, and synchronize
the directory. Reject an existing non-regular destination before staging. A
rename replaces a symlink itself rather than following it, but rejecting the
link makes the operator error explicit. The command prints a summary only, so
the native report is not copied into worker logs.

### Scope boundary

This change does not generate intent from a live Pijul repository and does not
upload the receipt through the worker API. Those are separate integration
steps. The documented worker profile can already declare the receipt as a
`review` report once the file exists.

## Risks

- A successful rename followed by a directory-sync failure returns an error even
  though the receipt exists. The caller must treat the receipt as retained and
  retry publication only after inspecting the path.
- The command can replace an earlier receipt at the same path. Callers that
  need immutable names must choose a unique output path; the receipt itself
  preserves the exact request and observation identities.
