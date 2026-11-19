## Phase 1: Pure fleet core

- [x] [parallel] Add `kiln-core::fleet::scope` with containment overlap and switch `implementation::swarm::evaluate_overlap` to it, with positive and negative tests. r[kiln.fleet.scope] r[kiln.implementation.swarm.overlap]
- [x] [parallel] [depends:kiln.fleet.scope] Add `fleet::claims` (registry, commands, conflicts against claims, attempts and foreign claims, leases, sweep, mirror effects) with tests. r[kiln.fleet.claims] r[kiln.fleet.mirror]
- [x] [parallel] [depends:kiln.fleet.scope] Add `fleet::session`, `fleet::decision` and `fleet::prompt` (observations, `plan_tick`, policy limits, prompts) with tests. r[kiln.fleet.dispatch] r[kiln.fleet.decisions]
- [x] [parallel] Admit `InteractiveWorktree` plans only for `herdr-interactive` work that permits `interactive_agent`, keeping Lattice rules, with tests. r[kiln.fleet.transport.scenario.admission]

## Phase 2: Shells and adapters

- [x] [parallel] Fix the swarm adapter: `groups`, `read_recent`, `IROH_NEWS_FROM`, and the implementer, campaign and mirror call sites, with scripted tests. r[kiln.implementation.swarm.transport]
- [x] [parallel] Add `kiln-adapter-herdr` with the Nickel inventory, bounded operations, placement and conformance tests against a scripted herdr. r[kiln.fleet.transport]
- [x] [parallel] [depends:kiln.fleet.claims] Persist `FleetState` in `kiln-implementer` with transactional claim, session and decision methods and reopen tests. r[kiln.fleet.claims] r[kiln.fleet.dispatch]
- [x] [parallel] Accept `kiln.herdr.candidate-receipt.v1` in `materialize_candidate` only for `herdr-interactive` plans, with tests. r[kiln.fleet.capture]

## Phase 3: Service

- [x] [serial] [depends:kiln.fleet.transport] Add `kiln-fleet`: the `kiln.fleet.v1` Nickel profile, the authenticated HTTP API, the stdio MCP server, client commands, the dispatch loop with capture, verification and publication invocation, and end-to-end tests with fake tailscale and herdr. r[kiln.fleet.api] r[kiln.fleet.dispatch] r[kiln.fleet.capture] r[kiln.fleet.decisions]
- [x] [serial] [depends:kiln.fleet.api] Package the `kiln-fleet` binary in the flake (the herdr adapter is a library it links), add the operator guide and changelog, and run the focused tests, clippy and Cairn validation. r[kiln.fleet.api]

## Phase 4: Live

- [x] [serial] [depends:kiln.fleet.api] Run `kiln-fleet` on britton-desktop with herdr headless sessions on britton-desktop and aspen3, register the MCP server for the agents, and dispatch one admitted work item to OMP through capture (`evidence/live-dispatch.md`). r[kiln.fleet.transport] r[kiln.fleet.capture]
