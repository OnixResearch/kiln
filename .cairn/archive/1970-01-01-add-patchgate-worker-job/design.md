## Context

The job script already performs the native work needed for an exact intent:

- it refuses an inbox that is behind main or has no pending change;
- reads `patchgate.json` and the selected policy from the target repository;
- reads the exact main/inbox native states;
- computes one canonical snapshot identity per pending change;
- builds the explicit dependency closure and selected pipeline identity.

The atomic `run` command now performs profile admission, planning, isolated
review, and receipt publication in one process.

## Decisions

### Invoke `run` once

Replace the separate `plan` and `review` calls with one
`kiln-patchgate-review run --profile PROFILE --intent INTENT --output RECEIPT`
call. The script reads the published receipt only to produce its bounded
summary. It no longer writes a partial receipt itself.

### Typed worker step

`config/patchgate-review-job.ncl` emits a `web-worker.ncl` `Step` with:

- `node_id` selected by the operator and matched to the pipeline review node;
- absolute Python, adapter, Patchgate, and Pijul executables;
- a checkout-relative script path and an absolute target repository;
- a checkout-relative receipt path declared as `kind = "review"`;
- explicit `PATH` so the script can resolve `b3sum`.

The worker’s existing report admission binds the uploaded bytes to the run,
attempt, execution, and node. The step does not introduce a new report kind or
upload path.

### Failure behavior

The script keeps its existing fail-closed checks. A missing pending change,
behind inbox, malformed native state, adapter failure, or destination rejection
exits nonzero, so the worker step fails and no review report is uploaded.
Missing evidence is not converted into a clean review.

## Risks

- The step depends on operator-supplied executable paths and `PATH`. A wrong
  path fails before review or at the first `b3sum` call; it cannot silently use
  a mutable worktree.
- The checkout must contain the script; the target must contain
  `patchgate.json`, the policy, and the native channels. The worker profile
  remains responsible for source selection and target exposure.
- The current `execution.read_only_mounts` policy admits only reviewed runtime
  roots. This step therefore requires an account-authority worker until a
  bounded target mount is added as separate work.
- This change does not prove a live Patchgate/Pijul deployment; the fake-native
  smoke verifies orchestration and receipt flow only.
