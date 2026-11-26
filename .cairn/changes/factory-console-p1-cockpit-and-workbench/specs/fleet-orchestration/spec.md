## ADDED Requirements

### Requirement: Live agent pane observation, machine slot census, foreign claims, and dispatch pause

r[kiln.fleet.live_pane_and_machine_census]
`kiln-fleet` MUST expose authenticated HTTP endpoints for live operator control: `GET /v1/machines` (returning `HerdrInventory`, live `HerdrClient::occupancy()`, explicit `Unavailable(HerdrFailure)` errors, and machine drain state), `POST /v1/machines/{name}/drain`, `GET /v1/attempts/{id}/pane?lines=N` (bounded `N <= 1000`, wrapping `HerdrClient::read`), `POST /v1/attempts/{id}/prompt`, `POST /v1/attempts/{id}/interrupt`, `GET /v1/claims?include=foreign,expired` (exposing `SwarmLink::foreign()`, mirror state, and recently expired claims), `GET /v1/dispatch/status` (exposing `TickReport` deferral reasons and `Running | Paused` state), and `POST /v1/dispatch/pause` / `POST /v1/dispatch/resume`. `crates/kiln-web-service/src/fleet.rs` MUST proxy these routes with strict path and parameter validation.

#### Scenario: Operator inspects blocked agent pane and steers without SSH

r[kiln.fleet.live_pane_and_machine_census.scenario.pane_read]
- GIVEN a running attempt in `blocked_question` state on an available `herdr` host
- WHEN the operator opens the Attempt in the Slide-Over Inspector and fetches `GET /api/fleet/attempts/{id}/pane?lines=200`
- THEN `kiln-fleet` MUST return bounded plain-text pane lines from `HerdrClient::read`
- AND the operator MUST be able to send an answer/prompt or interrupt through the guarded control route

#### Scenario: Offline host and foreign claim conflicts remain explicit

r[kiln.fleet.live_pane_and_machine_census.scenario.offline_and_foreign]
- GIVEN one unreachable `herdr` machine returning `HerdrFailure` and one active `ForeignClaim` blocking dispatch
- WHEN the console loads `GET /api/fleet/machines` and `GET /api/fleet/claims?include=foreign,expired`
- THEN the offline machine MUST display `Unavailable` with its error reason rather than `0/0` idle slots
- AND the blocking `ForeignClaim` and its mirror status MUST be surfaced explicitly
