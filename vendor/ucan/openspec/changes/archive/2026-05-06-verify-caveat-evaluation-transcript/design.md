# Design

## Verified primitive
Add `verified_logic::ucan_caveat_evaluation` with:

- a runtime fact carrying `policy_present` and `satisfied`;
- an acceptance predicate requiring every fact to have both booleans true;
- a first-failed-index helper returning the first missing-policy or unsatisfied-policy index.

## UCAN bridge
Expose `VerifiedCaveatEvaluationFact`, `VerifiedCaveatEvaluationTranscript`, `caveat_evaluation_transcript_accepts`, and `caveat_evaluation_first_failed_index` in `src/verified.rs`.

## Integration
`evaluate_capability_caveats` will emit facts while preserving existing denial reasons. The bridge only checks the shell-emitted finite transcript; it does not claim policy lookup, policy behavior, or caveat semantics.
