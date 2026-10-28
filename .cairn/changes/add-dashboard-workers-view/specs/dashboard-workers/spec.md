# Dashboard Workers View

## ADDED Requirements

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

The dashboard MUST integrate a Workers page with bounded metadata search, slot-state filtering, exact target selection, assigned-project/current-run inspection and explicit refresh. Navigation, filters and selection MUST restore from the URL and browser history on desktop/mobile. Selection MUST remain exact independently of filters; missing targets MUST NOT silently select a replacement. Loading/errors/no-match states and unknown liveness MUST remain explicit. Clearing the session MUST remove worker data and delayed responses MUST NOT repopulate a cleared or superseded page.

#### Scenario: Restore and inspect an exact target

r[kiln.dashboard.workers.view.scenario.restore]

- GIVEN a readable session with configured worker targets
- WHEN the operator opens Workers, searches or filters, selects a target, reloads or uses browser history and follows an exact run link
- THEN the visible inventory and exact selection restore without substitution, controls work at desktop/mobile widths and slot availability remains distinct from physical liveness

#### Scenario: Errors and session clearing

r[kiln.dashboard.workers.view.scenario.cleared]

- GIVEN worker data or a delayed inventory request
- WHEN access fails, the route changes or the session is cleared
- THEN unavailable/no-match states do not imply no workers or healthy workers, private data is erased and stale responses cannot restore it
