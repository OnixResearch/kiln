# Visual Identity Specification

## Purpose

Defines the `visual-identity` capability.

## Requirements

### Requirement: One shared Kiln identity layer

r[kiln.identity.shared]
Every Kiln browser surface (the live console, the offline catalog, the offline report and the hosted bridge page) MUST take its palette, type stacks, scale, radii, elevation, motion and brand mark from one shared identity stylesheet with a single semantic token vocabulary. A surface stylesheet MUST NOT declare a palette colour of its own and MUST NOT use a token that the identity or that stylesheet does not define. The identity MUST NOT reference a remote font, image or other remote asset, and offline documents MUST stay deterministic and self-contained.

#### Scenario: Every surface carries the identity

r[kiln.identity.shared.scenario.surfaces]
- GIVEN the live console, a rendered catalog, a rendered report and the hosted bridge page
- WHEN each is opened in a browser in light and in dark appearance
- THEN each MUST resolve the shared palette tokens and show the same brand mark, and no request MUST leave the document's own origin

#### Scenario: A palette literal or undefined token in a surface stylesheet

r[kiln.identity.shared.scenario.ownership]
- GIVEN a surface stylesheet that declares a colour literal or uses a token defined nowhere
- WHEN the identity check runs
- THEN it MUST fail and name the stylesheet and the offending declaration

### Requirement: Readable identity in both appearances

r[kiln.identity.contrast]
In light and in dark appearance, identity text colours MUST reach a contrast ratio of at least 4.5:1 on every surface colour they are drawn on. Each status colour MUST reach 4.5:1 on its own tinted background and on panel surfaces. Primary-action text MUST reach 4.5:1 on the primary fill and its hover fill. The focus ring and the input boundary MUST reach 3:1 against the surfaces they sit on. Action emphasis MUST remain a different colour from the success, failure, running and warning status colours.

#### Scenario: Declared palette meets the contrast floor

r[kiln.identity.contrast.scenario.palette]
- GIVEN the shared identity stylesheet
- WHEN the contrast check evaluates each declared text, status, action, focus and boundary pair in both appearances
- THEN every pair MUST meet its ratio

#### Scenario: A palette edit below the floor

r[kiln.identity.contrast.scenario.regression]
- GIVEN a palette in which one text or status colour falls below its ratio in one appearance
- WHEN the contrast check runs
- THEN it MUST fail and name the pair, the appearance and the measured ratio

### Requirement: Touch-sized controls on narrow offline and hosted pages

r[kiln.identity.touch_targets]
At viewport widths of 960px and narrower, every button, link, field, select and disclosure heading of the offline catalog, the offline report (opened alone or embedded in a catalog) and the hosted bridge page MUST be at least 44px tall, and the document MUST NOT overflow horizontally. Compact density MUST NOT reduce these controls below 44px. The rows of the pinned catalog index widget are exempt; the project cards and the HTML navigation MUST reach the same histories through controls of the required size.

#### Scenario: Phone and tablet widths

r[kiln.identity.touch_targets.scenario.narrow]
- GIVEN a rendered catalog, a rendered report and the hosted bridge page at 320px and at 768px, in comfortable and in compact density, with and without JavaScript
- WHEN each page is shown, and when the catalog's navigation, search results and keyboard help are opened
- THEN every button, link, field, select and disclosure heading MUST be at least 44px tall and the document MUST NOT scroll horizontally
