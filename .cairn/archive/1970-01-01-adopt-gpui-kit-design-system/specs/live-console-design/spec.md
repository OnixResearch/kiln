## ADDED Requirements

### Requirement: GPUI Kit design system in the live console

r[kiln.web.design_system]
The live console MUST draw every icon from the Lucide set of a pinned GPUI Kit revision, served from Kiln's own origin. The console's stylesheets MUST use a single semantic token vocabulary aligned with GPUI Kit's theme names, with no legacy token aliases. Pages MUST conform to GPUI Kit's Design Guides, except for documented web deviations. Navigation links, 44px mobile touch targets, visible focus and the existing route, authority and confirmation contracts MUST be preserved.

#### Scenario: Reproducible icon provenance

r[kiln.web.design_system.scenario.provenance]
- GIVEN the pinned GPUI Kit revision and the reviewed icon list
- WHEN the import script runs twice, or runs against a different revision or tampered source
- THEN the generated icon data and license MUST be byte-identical across runs, and a mismatched revision, NAR identity or missing icon MUST be refused before any file changes

#### Scenario: Icons without remote requests

r[kiln.web.design_system.scenario.local]
- GIVEN any console page
- WHEN it renders in a browser
- THEN every icon MUST render from local data, and no request MUST leave the console origin
