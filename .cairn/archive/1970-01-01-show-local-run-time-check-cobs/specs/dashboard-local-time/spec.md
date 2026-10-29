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
