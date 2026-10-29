# Live Console Design Specification

## Purpose

Defines the `live-console-design` capability.

## Requirements

### Requirement: Cohesive live console presentation

r[kiln.web.visual_design]
The live workspace MUST present a cohesive off-white/charcoal console with acid-yellow action emphasis, crisp framed surfaces, compact labelled navigation and consistent typography across CI and work views. Light, dark, system appearance and compact density MUST remain functional. The visual change MUST preserve existing routes, identity, state, authority, confirmation and keyboard contracts, and MUST NOT introduce remote asset requests or fabricated observations.

#### Scenario: Desktop console and appearance

- GIVEN an authenticated workspace with retained CI observations
- WHEN the operator navigates CI and work views and switches appearance or density
- THEN labels, statuses and real observations MUST remain readable, controls operable and semantic success/error distinct from action emphasis

#### Scenario: Narrow and keyboard interaction

- GIVEN a 320px viewport or keyboard-only navigation
- WHEN the operator opens navigation, creation, filters or the command palette
- THEN controls MUST remain reachable, touch controls at least 44px high on mobile, focus visible, and content contained without document-level horizontal overflow
