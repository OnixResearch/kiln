## ADDED Requirements

### Requirement: Kiln technical graphics standard

r[kiln.identity.graphics_standard]
Every Kiln browser surface MUST follow one technical graphics standard, declared in the shared identity stylesheet and modelled on the NASA Graphics Standards Manual (NHB 1430.2) and engineering-drawing practice. Surfaces MUST be neutral paper and ink, inverted in dark appearance. One identity red MUST be reserved for the brand mark, the index block of the page title rule and the request-progress line; it MUST NOT mark a status, an action or a selection. The single primary action of a view MUST be a solid ink block. Reading text MUST use a Helvetica-class grotesque, and identifiers, numbers and technical labels (section eyebrows, column heads, field labels and fact terms) MUST use a monospace, each named only from locally installed typefaces. Geometry MUST be square: every radius tier MUST be zero and no stylesheet MAY declare a literal radius except the brand mark's own arch. Surfaces in the page MUST be flat, structured by hairlines and ink frames; only overlays MAY be elevated, and only by an unblurred offset shadow. Status colours MUST keep their meanings.

#### Scenario: Every surface follows the standard

r[kiln.identity.graphics_standard.scenario.surfaces]
- GIVEN the identity stylesheet and every stylesheet that the live console, the offline catalog, the offline report and the hosted bridge page load after it
- WHEN the standard check runs
- THEN identity red MUST appear only in the brand mark, the title rule and the progress line, every radius tier MUST be zero, and no literal radius other than the mark's arch and no blurred shadow MUST be declared

#### Scenario: A departure from the standard

r[kiln.identity.graphics_standard.scenario.regression]
- GIVEN a stylesheet that paints an action or a status with identity red, rounds a corner, declares a non-zero radius tier or blurs a shadow
- WHEN the standard check runs
- THEN it MUST fail and name the stylesheet and the offending rule or token

## MODIFIED Requirements

### Requirement: Readable identity in both appearances

r[kiln.identity.contrast]
In light and in dark appearance, identity text colours MUST reach a contrast ratio of at least 4.5:1 on every surface colour they are drawn on. Each status colour MUST reach 4.5:1 on its own tinted background and on panel surfaces. Primary-action text MUST reach 4.5:1 on the primary fill and its hover fill. The focus ring, the input boundary and the ink frame of panels and controls MUST reach 3:1 against the surfaces they sit on, and a selected row's boundary MUST reach 3:1 on the row's own fill. Identity red MUST reach 3:1 against the navigation rail and the page, the brand mark's door MUST reach 3:1 against identity red, and the lit door MUST reach 3:1 against the idle door. Action emphasis MUST remain a different colour from the success, failure, running and warning status colours, and identity red MUST remain a different colour from the failure colour.

#### Scenario: Declared palette meets the contrast floor

r[kiln.identity.contrast.scenario.palette]
- GIVEN the shared identity stylesheet
- WHEN the contrast check evaluates each declared text, status, action, focus, frame, selection and brand-mark pair in both appearances
- THEN every pair MUST meet its ratio, and every pair of colours that must stay distinct MUST stay apart

#### Scenario: A palette edit below the floor

r[kiln.identity.contrast.scenario.regression]
- GIVEN a palette in which one text or status colour falls below its ratio in one appearance, or in which the failure colour is moved onto identity red
- WHEN the contrast check runs
- THEN it MUST fail and name the pair, the appearance and the measured ratio or distance
