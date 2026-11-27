## ADDED Requirements

### Requirement: Structured Plan Review annotation and receipt surface in the console

r[kiln.implementation.plan_review_console_surface]
The console MUST render structured implementation plans (`docs/plan-review.md`) inside the Slide-Over Inspector and `2.1 Delivery & Cairn` with step-level annotations, accept/request-changes feedback controls, and recorded plan-review receipt digests bound to the exact plan revision. Accepting or rejecting a plan MUST fail closed when the underlying plan revision has changed since observation.

#### Scenario: Stale plan revision blocks acceptance

r[kiln.implementation.plan_review_console_surface.scenario.stale_plan]
- GIVEN an operator reviewing a `plan_not_accepted` item whose plan revision changed concurrently
- WHEN the operator submits plan acceptance for the prior revision
- THEN the service MUST reject the stale acceptance with an explicit revision-mismatch diagnostic and require inspecting the new plan revision
