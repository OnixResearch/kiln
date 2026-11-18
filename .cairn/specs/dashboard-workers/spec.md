# Dashboard Workers Specification

## Purpose

Defines the `dashboard-workers` capability.

## Requirements

### Requirement: Bounded read-only worker-target inventory

r[kiln.dashboard.workers.inventory]

The live service MUST expose authenticated viewer/operator GET /api/workers over admitted workflow targets, bounded by existing project/run limits. Targets MUST group equal provider-profile, endpoint and platform bindings, use deterministic Kiln-owned identities, and preserve distinct bindings. The response MUST expose platform and assigned projects without endpoint paths, local repository paths, credential material, UID policy or executable arguments. Successful reads MUST perform no provider effects or state mutation. Unsupported queries/bodies and worker-role inventory reads MUST fail closed while preserving existing denial-audit behavior.

#### Scenario: Shared and distinct execution bindings

r[kiln.dashboard.workers.inventory.scenario.grouping]

- GIVEN several admitted projects including shared and distinct provider/endpoint/platform bindings
- WHEN a viewer or operator reads the inventory
- THEN shared targets appear once with their assigned projects, distinct targets retain distinct identities, ordering is deterministic and private binding fields are absent

#### Scenario: Unauthorized or malformed reads

r[kiln.dashboard.workers.inventory.scenario.denied]

- GIVEN worker credentials, a query parameter, a GET body or a non-read method
- WHEN inventory access is attempted
- THEN the existing role/API admission rejects the operation without provider effects or inventory mutation

### Requirement: Slot occupancy without invented liveness

r[kiln.dashboard.workers.capacity]

The inventory MUST distinguish configured project slots, enrolled slots, occupied slots, free slots and queued work using the existing queue admission predicate. Nonterminal ambiguous executions MUST continue to hold their slot. Storage failure MUST preserve the existing 503 admission guard and MUST NOT report usable free slots. Latest execution observations MUST retain exact run identity and known timing and MUST NOT be replaced by a later queued-only request, even when that request is cancelled before dispatch. Missing observations and physical worker liveness MUST remain unknown; a free configured slot MUST NOT claim online status, physical worker count or guaranteed dispatch.

#### Scenario: Enroll, occupy, queue and release

r[kiln.dashboard.workers.capacity.scenario.transitions]

- GIVEN a configured target before enrollment and then an enrolled project with running and queued work
- WHEN snapshots are read before enrollment, during execution and after terminal completion or cancellation
- THEN enrolled/free/occupied/queued slot counts follow actual durable state, and the target and run identities remain exact

#### Scenario: Ambiguous work or unavailable state

r[kiln.dashboard.workers.capacity.scenario.unknown]

- GIVEN an ambiguous capacity-holding execution, absent historical observation or failed durable storage
- WHEN the target inventory is projected
- THEN ambiguity never releases the occupied slot, absent facts remain null/unknown, failed storage returns unavailable rather than capacity and no worker heartbeat or online assertion is fabricated

### Requirement: Interactive session-safe Workers page
r[kiln.dashboard.workers.view]
The dashboard MUST integrate a read-only Workers page over the bounded target inventory, with a readable full-width register, immediate metadata/slot-state filtering, exact target selection, assigned-project/current-run inspection and explicit refresh. The register MUST name targets by their assigned project labels where available and keep provider/platform facts and latest retained execution inspectable without leading with target hashes. A selected target MUST show readable capacity, heartbeat and execution facts before its exact binding identities. Navigation, filters and selection MUST restore from the URL and browser history on desktop/mobile. Selection MUST remain exact independently of filters; missing targets MUST NOT silently select a replacement. Loading/errors/no-match states and unknown liveness MUST remain explicit. Free project slots or a prior run outcome MUST NOT imply current worker health, physical worker count or guaranteed dispatch. Clearing the session MUST remove worker data, and delayed responses MUST NOT repopulate a cleared or superseded page.

#### Scenario: Restore and inspect an exact target
r[kiln.dashboard.workers.view.scenario.restore]
- GIVEN a readable session with configured worker targets
- WHEN the operator filters, selects a target, reloads or uses browser history, returns to the register and follows an exact run link
- THEN inventory filters and exact selection MUST restore without substitution, the return path MUST retain the filters and restore usable focus, and slot availability MUST remain distinct from physical liveness

#### Scenario: Errors and session clearing
r[kiln.dashboard.workers.view.scenario.cleared]
- GIVEN retained worker data or a delayed inventory request
- WHEN access fails, the route is superseded or the session is cleared
- THEN unavailable/no-match states MUST NOT imply no workers or healthy workers, private data MUST be erased on session clearing and stale responses MUST NOT restore it

#### Scenario: Scan and filter the target register
r[kiln.dashboard.workers.view.scenario.register]
- GIVEN several targets with shared/long labels, distinct provider/platform bindings and varied slot states
- WHEN the reader opens Workers, types metadata or changes the slot-state filter
- THEN readable target rows MUST be ordered deterministically, match locally with a count, preserve distinct exact identities and selected targets, and restore their filters from the URL without a read for every keystroke

#### Scenario: Read a target before its identifiers
r[kiln.dashboard.workers.view.scenario.detail]
- GIVEN an exact target with assigned projects, current-run IDs and a latest execution or missing recorded observations
- WHEN the reader opens it at desktop or narrow widths
- THEN its readable title, slot/heartbeat state, capacity, latest execution and assigned work MUST precede secondary exact binding facts; exact run/project navigation and full target identity MUST remain reachable, and missing timing or counts MUST NOT become zero or now

#### Scenario: Keep heartbeat evidence distinct from admission slots
r[kiln.dashboard.workers.view.scenario.heartbeat]
- GIVEN targets with online, offline/expired or absent heartbeat observations, including monitoring configured before any admitted report and a noncurrent snapshot with positive reported free slots
- WHEN their register rows and details render
- THEN admission slots and heartbeat facts MUST remain distinct, absent reports MUST NOT be labelled expired, unknown/unrecognized liveness MUST be explicit, noncurrent reported free slots MUST NOT establish currently available capacity, and no previous run outcome MAY imply worker health
