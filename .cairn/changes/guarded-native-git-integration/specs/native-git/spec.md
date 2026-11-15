# Native Git Delivery Delta

## ADDED Requirements

### Requirement: Guarded native Git integration

r[kiln.implementation.native_git] Kiln MUST represent native Git publication and integration using a versioned contract distinct from `kiln.implementation-publication.v1` and `kiln.implementation-integration.v1`. A native intent and receipt MUST bind repository authority, exact enrolled remote identity, branch and target refs, expected previous target, work identity/version, attempt/fence, accepted candidate/tree, handoff/verification/review/check/policy identities, provider profile and operation identity. A receipt MUST identify the exact resulting remote ref and commit and MUST NOT contain a pull-request identity.

A route MUST persist each intended branch/target effect before transmission. Restart and lost acknowledgement MUST use read-only exact remote inspection; no pending or unknown operation may be blindly retried. Acceptance MUST recheck current work, authority, policy, reviews, candidate and fence. If the target has advanced before preparation, the integrator MUST create a new exact candidate and obtain fresh applicable checks and review. A local observation MUST NOT be represented as remote completion.

#### Scenario: Native receipt has exact candidate lineage

- GIVEN a currently authorized verified candidate and enrolled remote
- WHEN exact remote read-back confirms the guarded target result
- THEN Kiln records the native operation, prior target, candidate, checks, review, policy, resulting commit and ref without inventing a PR identity.

#### Scenario: Unknown effect survives restart

- GIVEN branch publication or target update may have reached the server but its acknowledgement was lost
- WHEN the durable consumer restarts
- THEN it inspects exact remote state and accepts only the exact operation result, otherwise remains pending or blocked without retransmission.

#### Scenario: Target advanced before preparation

- GIVEN a fresh target observation differs from the candidate's expected target
- WHEN integration is prepared
- THEN old checks and reviews are not reused; the new exact integration candidate requires applicable fresh evidence.

#### Scenario: Stale authority or substituted identity

- GIVEN a changed fence, revoked authority/policy/review, wrong repository/remote, moved branch or mismatched candidate
- WHEN native publication/integration is admitted
- THEN no effect or accepted receipt is produced.

#### Scenario: Remote rejects non-fast-forward or protection

- GIVEN target movement, non-fast-forward history, a server protection rejection or an unsupported transport/strategy
- WHEN guarded integration is attempted
- THEN Kiln preserves the explicit blocked/unsupported/unknown observation and never force-pushes or falls back to PR creation.

#### Scenario: Cairn lifecycle consumes native receipt

- GIVEN a confirmed native integration receipt bound to the exact lifecycle candidate
- WHEN #47 reconciles completion
- THEN the lifecycle consumer admits that receipt without requiring an issue or PR identity.
