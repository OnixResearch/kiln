## ADDED Requirements

### Requirement: Campaign index endpoint for operator selection

r[kiln.implementation.campaign.index_route]
The web service MUST expose `GET /api/implementation/campaigns` returning a bounded, ordered index of active and recent implementation campaigns with their identifiers, current status, pending human-decision counts, and latest update timestamps. The Fleet and Workers views MUST populate a selectable campaign list from this endpoint instead of requiring operators to memorize and type raw campaign IDs.

#### Scenario: Operator selects a campaign from the live index

r[kiln.implementation.campaign.index_route.scenario.list]
- GIVEN one or more active or recent implementation campaigns in the store
- WHEN an operator opens Fleet or Workers and the console fetches `GET /api/implementation/campaigns`
- THEN each campaign MUST appear in a selectable list with its status and pending decision count
- AND an empty store MUST render an explicit empty state rather than an error or fabricated campaign
