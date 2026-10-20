## Context

`VerificationLimits::max_proof_depth` bounds recursive proof traversal. The shell currently rejects `depth > max_depth` directly. In verified-logic, `ucan_delegation_depth::check_depth_budget(chain_len, max_depth)` proves the runtime check matches a spec where a single-token chain has depth zero and delegation depth is `chain_len - 1`.

## Decision

Expose `proof_depth_within_budget(depth, max_depth) -> bool` from `src/verified.rs`. The shell depth value is already the number of proof hops from the leaf, so the bridge converts it to `chain_len = depth + 1` with checked/saturating-safe handling and calls the verified primitive.

`enforce_proof_depth` will reject when the bridge returns false and retain the existing `VerificationLimitError::ProofDepthExceeded` shape.

## Non-Goals

- No change to proof count or compact-token byte limits.
- No new resolver/store behavior.
- No full structural-chain replacement; this slice is only the exact finite depth-budget predicate.

## Risks

- Off-by-one mismatch between shell depth and verified chain length. Mitigate with focused tests covering leaf depth, exact budget, and over-budget rejection.
- Archive sync can clobber existing scenarios. Inspect and repair canonical spec diffs after archive.
