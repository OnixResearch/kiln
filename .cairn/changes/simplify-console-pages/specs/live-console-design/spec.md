## ADDED Requirements

### Requirement: Task-first console hierarchy

r[kiln.web.console_page_priority]
The console MUST preserve exact destination and detail routes while prioritizing the work each page supports. Routine actions, failures, and unavailable observations MUST remain visible. Supporting inventory and provenance MAY be grouped separately without changing their ownership or linking them by inference.

#### Scenario: Open a secondary destination directly
- GIVEN a destination in a collapsed navigation group
- WHEN a reader opens its exact route
- THEN the group opens and the destination is marked current
