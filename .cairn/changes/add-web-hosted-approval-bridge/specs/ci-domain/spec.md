## MODIFIED Requirements

### Requirement: Distinct attempts and unknown outcomes
r[kiln.domain.attempts] Every retry MUST create a distinct attempt identity, and an ambiguous provider observation MUST remain an unknown outcome until an admitted reconciliation or operator decision resolves it. Reconciliation MAY restore Pending only when an admitted nonterminal observation names the exact live execution reference, matching any retained reference. This transition MUST NOT dispatch or create another attempt. A missing or substituted reference MUST be refused during decision and event replay. Cancellation intent MUST NOT become a completed cancellation outcome.

#### Scenario: Submission connection fails after dispatch
r[kiln.domain.attempts.scenario.unknown]
- GIVEN a provider dispatch that might have reached the provider
- WHEN the shell reports a connection failure without provider proof
- THEN Kiln MUST record an unknown outcome and MUST NOT classify the attempt as not started.

#### Scenario: Exact execution is still running after uncertainty
r[kiln.domain.attempts.scenario.live_reconciliation]
- GIVEN an unknown attempt and a current admitted Running or Paused observation of its exact execution
- WHEN reconciliation restores Pending
- THEN the original attempt becomes cancellable without another dispatch, while missing or changed execution references are refused and cancellation remains pending until the actual provider result.
