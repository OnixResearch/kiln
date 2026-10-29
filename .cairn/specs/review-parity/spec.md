# Review Parity Specification

## Purpose

Defines the `review-parity` capability.

## Requirements

### Requirement: Owned paths and protected target authority

r[kiln.review.parity.ownership]
Review policy MUST support path/team owner requirements and automatic review requests. Current non-author owner approvals MUST be required for matching changed paths. Every service target write MUST pass the same policy and exact-target authority boundary; external unmanaged writers MUST be explicitly identified as outside service enforcement.

#### Scenario: Changed owned path

- GIVEN a path owned by an admitted team and a current revision
- WHEN an unrelated reviewer approves or the revision changes
- THEN owner approval MUST remain unmet until an eligible non-author owner approves that current revision

### Requirement: Natural contribution intake

r[kiln.review.parity.intake]
Authenticated source intake MUST create and update stable changes from explicitly admitted source branches and exact commit observations. Replayed events MUST NOT create duplicate revisions; stale, malformed, unauthorized and mismatched source inputs MUST fail before provider effects.

#### Scenario: Branch update and replay

- GIVEN an admitted source branch and its authenticated author
- WHEN its exact commit is submitted and later updated or replayed
- THEN the same change MUST retain immutable revision history and a replay MUST NOT duplicate the update

### Requirement: Usable and safe review ergonomics

r[kiln.review.parity.ergonomics]
Review pages MUST offer syntax highlighting, safe Markdown, whitespace filtering and applyable exact anchored suggestions. Applying a suggestion MUST create a new immutable review revision without directly advancing the target, and MUST invalidate previous approval authority. Filtered displays MUST retain valid exact line anchors.

#### Scenario: Stale or malicious suggestion

- GIVEN a suggestion anchored to one exact textual revision and line range
- WHEN the revision changes or comment text contains active HTML
- THEN stale application MUST fail closed and rendering MUST NOT execute supplied HTML

### Requirement: Delivered review attention

r[kiln.review.parity.delivery]
Published mentions and principal-owned subscriptions MUST produce attributable notifications without disclosing drafts. External review delivery MUST use explicitly admitted recipients and destinations, bounded requests and durable identities and outcomes across restart.

#### Scenario: Published mention and restart

- GIVEN a subscribed or mentioned admitted principal and a private draft
- WHEN the draft is published and delivery is interrupted
- THEN no draft content MUST have been sent before publication and recovery MUST retain the exact notification identity and observed delivery outcome

### Requirement: Candidate-bound merge queue

r[kiln.review.parity.queue]
The merge queue MUST construct exact integration candidates against the current target, require canonical CI for those candidate identities, reevaluate approvals/policy/dependencies and exact targets before submission, and retain bounded durable queue and provider state through restart. Changed targets or candidates MUST invalidate prior evaluations. Unsupported providers MUST reject before effects.

#### Scenario: Queue target movement and crash

- GIVEN an approved queued change with a tested integration candidate
- WHEN the target moves or a submission outcome is ambiguous
- THEN stale CI MUST NOT authorize submission and recovery MUST reconcile the original operation without blindly repeating external effects

### Requirement: Required CI workflow qualification

r[kiln.review.parity.qualification]
Verification MUST exercise real canonical required-CI success through current-revision approval and independently observed disposable target submission, plus failing/stale CI, restart recovery, desktop/mobile browser interaction and applicable repository gates. Evidence MUST distinguish local qualification from deployment or publication.

#### Scenario: Required CI success and failure

- GIVEN disposable source and target repositories with required CI enabled
- WHEN real candidate execution succeeds or fails
- THEN only the exact successful approved candidate MAY submit and the independently observed target MUST match its retained result
