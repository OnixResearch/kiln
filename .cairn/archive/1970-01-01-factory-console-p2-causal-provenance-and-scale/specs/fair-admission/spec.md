## ADDED Requirements

### Requirement: Live Fair-Admission governance and AttemptMetrics visibility in the console

r[kiln.admission.live_console_governance]
`kiln-web-service` MUST expose `GET /api/implementation/admission` projecting the live fair-admission tenant tree, quotas, shares, `ServingBounds`, held debits, and `AttemptMetrics` effort summaries (tokens, turns, wall-time burn) without conflating transfers with consumption or unobserved data with zero. `3.1 Fleet & Workers` MUST render this projection and route any authorized quota/share updates through owner commands with explicit freshness and authority checks.

#### Scenario: Fair-admission shares and unobserved burn render honestly

r[kiln.admission.live_console_governance.scenario.honest_metrics]
- GIVEN two active tenants with distinct fair-admission shares and one attempt lacking token telemetry
- WHEN the operator views the Fair-Admission & Effort panel in `3.1 Fleet & Workers`
- THEN each tenant's exact share, held debit, and serving bound MUST be displayed separately from transfers
- AND the attempt lacking token telemetry MUST display `not recorded` rather than `0`
