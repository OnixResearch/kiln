## ADDED Requirements

### Requirement: Unified Attention aggregation, campaign wake, and termination reconciliation routes

r[kiln.implementation.campaign.unified_attention_and_recovery]
`kiln-web-service` MUST expose `GET /api/implementation/attention` aggregating all human-required escalations (`UntrustedRunApproval`, `FleetSessionDecision`, `CampaignDecision`, `PlanReviewGate` for `plan_not_accepted`, `UnknownTerminationOrDispatch`, and `ExpiringOrConflictClaim`) with severity, target identities, context preview, TTL, and allowed actions. It MUST also expose `POST /api/implementation/campaigns/{id}/wake` and `POST /api/implementation/reconcile-termination` so operators can wake deferred campaigns, review/accept blocked plans inline in the Slide-Over, and clear ambiguous termination guards without dropping to the CLI.

#### Scenario: Ambiguous termination and blocked plan resolve from Attention

r[kiln.implementation.campaign.unified_attention_and_recovery.scenario.resolve]
- GIVEN a campaign member blocked on `plan_not_accepted` and another blocked on an ambiguous termination guard
- WHEN the operator opens `GET /api/implementation/attention` in the Cockpit or Attention view
- THEN both items MUST appear with exact target identities and context previews
- AND invoking the inline plan gate or `POST /api/implementation/reconcile-termination` MUST validate exact identities before clearing the guard
