# Why

Issue #64 identifies a review-admission predicate mismatch: a typed pass from historical, wrong-project or ineligible CI can satisfy a newer selected run with no such check. The selected run and displayed evidence must agree before this revision is deployed.

## What Changes

- Bind configured typed requirements to the ordinary policy-selected run for the review project and its current applicable scope.
- Require a successful terminal run with current trusted/untrusted counting admission; retain exact integration-candidate binding and latest-run invalidation.
- Define monorepo mapping explicitly: typed requirements belong to the review project; other affected projects retain their independent ordinary CI requirements and cannot lend a typed pass.
- Report the actual selected typed run, with positive/negative regression coverage for every issue acceptance case.

## Impact

Pure change-review predicates and tests, operator documentation and source-bound local evidence. Existing policy and checklist wire shapes stay unchanged. No provider, source or lifecycle authority is created.

## Authority

The operator requested commit, push, deployment and continued issue completion. This package repairs the discovered admission defect before activation; it does not waive the existing traceability/release holds.
