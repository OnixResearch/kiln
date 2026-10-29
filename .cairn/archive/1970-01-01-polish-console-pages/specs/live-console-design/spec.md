## ADDED Requirements

### Requirement: Consistent, readable console pages

r[kiln.web.page_consistency]
Every live-console page MUST use the shared page frame and components. Each page MUST present one clear purpose and at most one primary action, with readable primary text and exact identifiers available but secondary. Explicit loading, empty, unavailable and error states MUST be shown for each data section. Boundary and non-claim statements MUST remain reachable, and MAY be placed behind a labelled disclosure when they would otherwise dominate the page. At 320, 768, 1440 and 1920px, in light and dark appearance, pages MUST avoid document-level horizontal overflow and meaning-hiding truncation. They MUST also keep visible focus and 44px mobile touch targets.

#### Scenario: Secondary page with retained data

r[kiln.web.page_consistency.scenario.data]
- GIVEN retained CI, work or review data
- WHEN the operator opens any console page at any supported width or appearance
- THEN the page MUST show its title, purpose and primary action first, render data with shared components, and keep exact identities and boundary statements reachable

#### Scenario: Empty or unavailable page section

r[kiln.web.page_consistency.scenario.states]
- GIVEN an empty, unconfigured or failed data source
- WHEN its page renders
- THEN the affected section MUST show an explicit state with guidance where possible, and no sample data or substituted selection MUST appear
