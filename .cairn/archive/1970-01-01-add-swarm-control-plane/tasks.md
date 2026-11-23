## Phase 1: Pure `kiln-core::fleet::swarm` Control Plane

- [x] [serial] Add directional `contains_scope` to `kiln_core::fleet::scope` and implement `crates/kiln-core/src/fleet/swarm/` (`SwarmRun`, `SwarmEnvelope`, `SwarmBudget`, `SwarmRole`, `SwarmMember`, `SwarmMemberState`, `SwarmBoard`, `mention_graph`, `admit_spawn`, `submit_verdict`, `evaluate_watchdog`, `record_watchdog_pass`, `apply_quota_failover`, `SwarmLedger`) with unit and adversarial tests. r[kiln.fleet.swarm.model] r[kiln.fleet.swarm.model.scenario.untrusted_board] r[kiln.fleet.swarm.model.scenario.mention_graph] r[kiln.fleet.swarm.delegation] r[kiln.fleet.swarm.delegation.scenario.scope_widening] r[kiln.fleet.swarm.delegation.scenario.verifier_read_only] r[kiln.fleet.swarm.delegation.scenario.overlap_and_bounds] r[kiln.fleet.swarm.verdict] r[kiln.fleet.swarm.verdict.scenario.repair_and_escalation] r[kiln.fleet.swarm.watchdog] r[kiln.fleet.swarm.watchdog.scenario.wake_deadlock_stale] r[kiln.fleet.swarm.watchdog.scenario.quota_failover]

## Phase 2: Fleet Persistence, HTTP API, CLI, and MCP Surface

- [x] [serial] Persist `SwarmLedger` in `kiln_implementer::fleet::FleetState`, add `Implementer` swarm methods, register all new `Implementer` and `FLEET_API` routes in `ENTRY_POINTS`, and expose `/v1/swarms*` HTTP routes, `kiln-fleet swarm` CLI subcommands, and `kiln_swarm_*` MCP tools with integration tests. r[kiln.fleet.swarm.surface] r[kiln.fleet.swarm.surface.scenario.http_and_mcp]

## Phase 3: Console Swarm View & Verification

- [x] [serial] Add read-only `/api/fleet/swarms*` proxy routes, server-rendered `/fleet/swarms` and `/fleet/swarms/{id}` HTML inspection views with inline SVG replayable mention graph and strict HTML escaping, and live Fleet console integration in `kiln-web-service`, then pass formatting, Clippy, tests, `authority_coverage`, and Cairn gates. r[kiln.fleet.swarm.console] r[kiln.fleet.swarm.console.scenario.escaping_and_replay]
