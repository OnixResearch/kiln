## Context

The legacy lifecycle tree is complete, but current Cairn rejects a repository that has only `cairn/`. The checked policy and active documentation also name the legacy path.

## Decisions

### Decision: Use Cairn's preview-bound copy plan

**Choice:** Execute `cairn migrate cairn` only with the reviewed preview hash. Keep both receipts in the archived change.

**Rationale:** The command binds every source and destination path to BLAKE3 identities and rejects drift before mutation.

### Decision: Rebase policy before final validation

**Choice:** Snapshot the pinned upstream contracts and default policy. Keep Kiln traceability roots in a small Nickel overlay.

**Rationale:** A narrow overlay retains Kiln evidence roots without maintaining a full policy fork.

### Decision: Remove the legacy tree after parity checks

**Choice:** Verify migrated bytes, update active path references, and then remove `cairn/`.

**Rationale:** One lifecycle source prevents silent divergence. Historical prose can retain old path statements when it describes past facts.

## Tests

- Positive: the migration plan executes, strict validation passes, and active checks read `.cairn/`.
- Negative: a stale or replayed migration plan fails after the legacy source is removed.
- Regression: workspace tests, Clippy, traceability, and Nix checks pass without the old directory.

## Risks / Trade-offs

- Existing active changes move without gaining approval or completion status.
- The policy snapshot can drift from future Cairn revisions and requires an explicit update.

## Claim Boundaries

Migration proves copied path and byte identity under the selected plan. It does not prove requirement truth, CI behavior, provider effects, or release readiness.
