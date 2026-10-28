## ADDED Requirements

### Requirement: Browser-local recorded times alongside UTC

r[kiln.dashboard.time.local]
The live workspace MUST display recorded timestamps in the browser's local timezone with an explicit timezone label alongside the same instant in ISO UTC. Overview last-run completion MUST be visible. Missing, unsafe and invalid timestamps MUST remain unavailable; presentation MUST NOT change stored timing or infer missing observations.

#### Scenario: Local day differs from UTC

r[kiln.dashboard.time.local.scenario.zone]
- GIVEN one recorded run with a valid timestamp
- WHEN an admitted reader views its latest execution or overview in a timezone whose calendar date differs from UTC
- THEN the local date/time and timezone MUST match that instant and the exact UTC timestamp MUST remain visible

#### Scenario: Missing timing stays unavailable

r[kiln.dashboard.time.local.scenario.unknown]
- GIVEN a run with absent or invalid timing
- WHEN its timestamps are rendered
- THEN neither epoch zero nor the current time MUST replace the unavailable observation

### Requirement: Operated read-only COB availability

r[kiln.dashboard.cobs.operated]
The operated dashboard MUST read only the explicitly admitted public Kiln Git store, distinguish available empty inventory from unconfigured or unavailable storage, and preserve exact native identities and missing-object errors. Verification MUST exercise all six supported types through isolated positive and negative native fixtures. A service request MUST NOT create, import, sign, synchronize or publish objects.

#### Scenario: Existing store contains no Onix objects

r[kiln.dashboard.cobs.operated.scenario.empty]
- GIVEN an available explicitly selected public Kiln store with no supported Onix refs
- WHEN an admitted reader opens the COB browser and requests an absent exact object
- THEN inventory MUST be configured, complete and empty, exact lookup MUST return missing, and neither operation MUST mutate Git refs or CI state

### Requirement: Authorized Cairn COB creation

r[kiln.dashboard.cobs.population]
Under one explicit operator request, each active Cairn change MUST be importable as a signed `xyz.onixresearch.cairn-change` COB in the selected store, binding its proposal, design, tasks, metadata and delta-spec identities. Creation MUST use the operator's existing Radicle identity, MUST refuse a change that already has a COB, MUST leave every pre-existing ref, store `HEAD`, `packed-refs` and store config unchanged, and MUST NOT publish, synchronize, announce or mutate delegates.

#### Scenario: Signed import appears in the browser

r[kiln.dashboard.cobs.population.scenario.import]
- GIVEN an unchanged store and an active change without a COB
- WHEN the operator imports that change and reopens the browser
- THEN inventory MUST list one new object with its native operation ID and one observed ref tip, the exact artifact bytes MUST be readable in the operation actions, signatures MUST remain unverified, and a repeated import MUST be refused as an existing COB

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

