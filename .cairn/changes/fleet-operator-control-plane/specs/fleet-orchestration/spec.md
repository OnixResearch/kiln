## ADDED Requirements

### Requirement: Fleet composes exact operator observations and controls

r[kiln.fleet.operator_plane] Fleet MUST expose exact attempt inspection, operator attention and recovery, queue and capacity explanation, worker runtime evidence and campaign controls through their existing owners. It MUST preserve explicit unknown and unavailable states, and MUST NOT translate ambiguous dispatch into redispatch or provider observation into lifecycle authority.

#### Scenario: A blocked or unknown attempt can be inspected without crossing authority

r[kiln.fleet.operator_plane.scenario.stuck]
- GIVEN a Cairn change with a blocked or unknown execution
- WHEN an operator inspects it in Fleet
- THEN the exact owner observations and supported controls MUST be reachable
- AND unsupported or unobserved facts MUST be explicit rather than inferred
