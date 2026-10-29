# COB content presentation

## ADDED Requirements

### Requirement: Readable type-specific recorded content

r[kiln.dashboard.cobs.presentation]

The dashboard SHALL render the recorded content of all nine admitted COB types with type-appropriate labels and preserved text, retain exact operation/action framing, and SHALL NOT claim reduced lifecycle state or verified authority. Inventory titles SHALL use bounded root action metadata when present and fall back to the root commit summary when unavailable.

#### Scenario: Different native encodings

- GIVEN Cairn init/artifact/field actions, Radicle issue/patch actions and externally tagged job events
- WHEN their exact object is selected
- THEN the recorded documents, discussions, revisions, requests and results are readable without decoding JSON strings, with exact raw action content available separately

#### Scenario: Title outside action zero

- GIVEN an issue root whose body is in action zero and title in action one
- WHEN inventory is read and searched
- THEN the bounded recorded root title identifies the object without treating a later edit as authoritative state

#### Scenario: Unknown, malformed or hostile content

- GIVEN unsupported action shapes, malformed or oversized root metadata, concurrent operations or embedded HTML and URLs
- WHEN the object is inspected
- THEN valid retained raw content remains available, summary fallback is explicit, no concurrent winner is selected and no embedded executable content or external resource is activated
