## ADDED Requirements

### Requirement: Task-first page hierarchy

r[kiln.web.page_focus]
Each workspace destination MUST use its shared title and concise purpose to orient the operator. The overview MUST place current issues before aggregate health when a project needs attention. The workers page MUST prioritize configured execution targets over supplemental inventory while keeping the latter reachable in place. Existing routes, exact evidence, and command admission MUST be preserved.

#### Scenario: Diagnostic data remains accessible
- GIVEN a workers page with configured targets and supplemental observed inventory
- WHEN the operator opens the page, then expands the supplemental inventory
- THEN the configured execution targets are first and the observed inventory is reachable without leaving the page
