## ADDED Requirements

### Requirement: Combined Fleet UI surface and proxy exposure of existing metrics and version routes

r[kiln.fleet.unified_console_surface]
Kiln MUST NOT maintain a second divergent interactive HTML control plane in `crates/kiln-fleet/src/ui.html`. `kiln-fleet`'s `/` and `/ui` routes MUST combine into the unified `kiln-web-service` Factory Control Plane (redirecting to the configured console Fleet URL or returning a clear pointer when unconfigured). `crates/kiln-web-service/src/fleet.rs` MUST extend its `upstream_path` allowlist to proxy existing `kiln-fleet` `/v1/metrics` and `/v1/version` endpoints while continuing to reject unallowlisted or traversal paths.

#### Scenario: Standalone Fleet UI route points to unified console

r[kiln.fleet.unified_console_surface.scenario.combined]
- GIVEN a `kiln-fleet` server configured with the primary `kiln-web-service` console URL
- WHEN an operator requests `GET /` or `GET /ui` on `kiln-fleet`
- THEN the response MUST direct the operator to the unified Factory console rather than serving a separate bespoke `prompt()`/`alert()` HTML application

#### Scenario: Metrics and version proxy enforces allowlist

r[kiln.fleet.unified_console_surface.scenario.proxy]
- GIVEN an authenticated console operator requesting `/api/fleet/metrics` or `/api/fleet/version`
- WHEN `fleet.rs` forwards the request to `kiln-fleet`
- THEN `/v1/metrics` and `/v1/version` MUST be proxied cleanly
- AND unallowlisted paths or path-traversal attempts MUST be rejected before upstream dispatch
