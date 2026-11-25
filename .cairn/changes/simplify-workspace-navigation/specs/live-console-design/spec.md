## ADDED Requirements

### Requirement: Concise primary navigation

r[kiln.web.primary_navigation]
The shared workspace rail MUST expose Overview, Runs, CI projects, Issues and Code review directly, and group supplemental destinations in one More tools disclosure while keeping the existing Guide and all registered routes reachable.

#### Scenario: A secondary destination remains reachable
- GIVEN a default workspace with More tools closed
- WHEN an operator opens a secondary route, including via direct URL or command search
- THEN More tools opens and its link is marked current; returning to a primary route collapses the disclosure
