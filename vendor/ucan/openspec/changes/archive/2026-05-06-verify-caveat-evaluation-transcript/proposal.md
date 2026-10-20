# Verify caveat evaluation transcript admission

## Motivation
Caveat policy evaluation is intentionally caller-supplied, but the shell still owns the fail-closed aggregation rule: every caveat on a matched capability must have a policy and that policy must satisfy the caveat before authorization is allowed. Today that aggregation is ordinary Rust control flow.

## Change
Add a narrow verified transcript predicate over shell-emitted caveat evaluation facts (`policy_present`, `satisfied`) and route `evaluate_capability_caveats` through it. The predicate proves only finite fail-closed aggregation and first-failure ordering over supplied facts.

## Non-goals
Do not verify caveat payload parsing, arbitrary policy correctness, typed caveat runtime parsing, clocks, counters, policy lookup truth/provenance, authorization resource matching, token provenance, or cryptography.
