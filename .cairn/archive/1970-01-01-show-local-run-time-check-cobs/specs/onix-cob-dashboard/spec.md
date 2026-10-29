## MODIFIED Requirements

### Requirement: Bounded native COB observations
r[kiln.dashboard.cobs.read] Kiln MUST expose a read-only inventory and exact object history for its six registered Onix COB types and three registered Radicle observation types (`xyz.radicle.issue`, `xyz.radicle.patch`, `xyz.radworks.job`) from explicitly admitted local repositories, preserving native repository, type, object, ref-tip, author, recorded-time and operation identities under named bounds without substituting a Cairn reducer.

#### Scenario: Supported objects are present
r[kiln.dashboard.cobs.read.scenario.present]
- GIVEN local Cairn change, RFD, ADR, verification-obligation, release-decision or policy-exception COB refs, or native Radicle issue, patch or job refs
- WHEN an admitted reader requests inventory and one exact object
- THEN Kiln MUST preserve native identities and bounded operation/action content, including every observed ref tip and the recorded author/time
- AND it MUST label raw history as observations rather than reduced lifecycle, signature verification or authority
- AND types outside the registry MUST be skipped without failing the inventory.

#### Scenario: A store or cursor is unavailable
r[kiln.dashboard.cobs.read.scenario.unavailable]
- GIVEN an unavailable or malformed store, exceeded bound, moved ref or cursor bound to other filters
- WHEN a read is requested
- THEN Kiln MUST report explicit unavailability or reject the cursor rather than returning false empty success or a substituted object.

### Requirement: Integrated accessible COB view
r[kiln.dashboard.cobs.view] The live workspace MUST provide a COBs navigation view with repository/type/search filtering, stable bounded paging, exact URL-restored object selection and readable raw content/identity inspection using existing authentication and responsive shell conventions, showing each object's type label, title, author and browser-local recorded time.

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

