# Onix Cob Dashboard Specification

## Purpose

Defines the `onix-cob-dashboard` capability.

## Requirements

### Requirement: Bounded native COB observations
r[kiln.dashboard.cobs.read] Kiln MUST expose a read-only inventory and exact object history for its six registered Onix COB types from explicitly admitted local repositories, preserving native repository, type, object, ref-tip and operation identities under named bounds without substituting a Cairn reducer.

#### Scenario: Supported objects are present
r[kiln.dashboard.cobs.read.scenario.present]
- GIVEN local Cairn change, RFD, ADR, verification-obligation, release-decision or policy-exception COB refs
- WHEN an admitted reader requests inventory and one exact object
- THEN Kiln MUST preserve native identities and bounded operation/action content, including every observed ref tip
- AND it MUST label raw history as observations rather than reduced lifecycle, signature verification or authority.

#### Scenario: A store or cursor is unavailable
r[kiln.dashboard.cobs.read.scenario.unavailable]
- GIVEN an unavailable or malformed store, exceeded bound, moved ref or cursor bound to other filters
- WHEN a read is requested
- THEN Kiln MUST report explicit unavailability or reject the cursor rather than returning false empty success or a substituted object.

### Requirement: Read-only authorized policy boundary
r[kiln.dashboard.cobs.authority] Kiln MUST admit COB repository policy separately from canonical CI state and authorize only authenticated viewer/operator reads without accepting browser paths, signing, mutation, credentials or transport authority.

#### Scenario: A reader browses configured repositories
r[kiln.dashboard.cobs.authority.scenario.reader]
- GIVEN a separately admitted typed runtime policy and authenticated viewer or operator
- WHEN a configured repository is browsed
- THEN Kiln MUST read only the admitted local store without changing CI profile identity, journal or refs
- AND it MUST keep host paths and credentials out of browser responses.

#### Scenario: Unadmitted authority is requested
r[kiln.dashboard.cobs.authority.scenario.denied]
- GIVEN an anonymous or worker request, a write, unknown query field, unconfigured repository or malformed identity
- WHEN COB access is attempted
- THEN Kiln MUST reject it before repository effects.

### Requirement: Integrated accessible COB view
r[kiln.dashboard.cobs.view] The live workspace MUST provide an Onix COBs navigation view with repository/type/search filtering, stable bounded paging, exact URL-restored object selection and readable raw content/identity inspection using existing authentication and responsive shell conventions.

#### Scenario: A COB deep link is restored
r[kiln.dashboard.cobs.view.scenario.restore]
- GIVEN an admitted object selection and filters in the page URL
- WHEN the reader opens, reloads or navigates browser history
- THEN the page MUST restore that exact selection and filters without silently choosing another object
- AND object/action text MUST remain inert text on desktop and narrow screens.

#### Scenario: Private views are cleared
r[kiln.dashboard.cobs.view.scenario.cleared]
- GIVEN a pending COB read or a populated inspector
- WHEN the session is cleared or selection changes
- THEN old responses MUST NOT repopulate the current private view
- AND empty, unconfigured and unavailable sources MUST remain distinguishable.
