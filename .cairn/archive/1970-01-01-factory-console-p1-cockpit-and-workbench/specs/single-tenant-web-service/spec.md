## ADDED Requirements

### Requirement: Single multiplexed SSE event stream backed by a shared coordinator watcher

r[kiln.web.multiplexed_sse_events]
`kiln-web-service` MUST replace 1.5s CI short-polling (`/api/live`) and per-tab upstream Fleet polling loops (`/api/fleet/events`) with a single multiplexed persistent SSE stream per browser tab (`/api/events`) emitting typed `ci`, `fleet`, `cairn`, and `attention` invalidation/snapshot frames. A single shared background coordinator watcher inside `kiln-web-service` MUST fan out upstream `kiln-fleet` updates to all connected SSE clients so multiple browser tabs do not multiply upstream connections against `kiln-fleet`'s `MAX_CONNECTIONS = 32` limit or starve `kiln-web-service`'s `max_connections` bound.

#### Scenario: Multi-tab monitoring uses one upstream watcher and one SSE stream per tab

r[kiln.web.multiplexed_sse_events.scenario.fanout]
- GIVEN five browser tabs connected to `/api/events`
- WHEN `kiln-fleet` or the CI engine records a state transition
- THEN `kiln-web-service` MUST perform only one upstream observation loop and fan out the typed SSE frame to all five tabs
- AND closing a tab MUST release its SSE server thread promptly
