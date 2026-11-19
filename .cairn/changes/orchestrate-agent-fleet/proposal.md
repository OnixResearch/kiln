## Why

Britton's coding agents (OMP, Claude Code, Codex, Antigravity) run on britton-desktop, aspen3, aspen1 and aspen2. They coordinate by posting `CLAIM` and `RELEASE` articles to a private swarm newsgroup server. Kiln `03f57a8c` mirrors its own implementation attempts into that group and holds off on matching claims. The swarm only advises: any agent can post any claim, nothing expires unless its author releases it, overlaps are judged by etiquette, and nothing assigns work to an agent or follows it to a result. OnixResearch/kiln#94 also found three faults in the mirror:

- it reads the oldest 50 articles instead of the newest;
- it recognizes its own claims by a From address that every agent on a machine shares;
- it only matches identical scopes.

Britton asked for Kiln to orchestrate the agents. Kiln already owns work intake, claims with leases and fences, independent verification and verified publication. It lacks four things:

- a claim authority that agents can reach from other machines;
- a transport that runs an attempt in an interactive agent session;
- placement across machines;
- a path from an agent's commit to Kiln's candidate handoff.

## What Changes

- Fix the swarm mirror:
  - read the newest window, using the group high-water mark;
  - post with Kiln's own From address;
  - decide overlap by path containment, with a case-insensitive repository.
- Add a pure `fleet` core (`kiln-core::fleet`):
  - claim scopes and containment overlap;
  - a fleet claim registry with leases, fences, expiry and conflicts against fleet claims, active attempts and foreign swarm claims;
  - an interactive session ledger with a tick planner that turns observations into requested effects;
  - a decision ledger;
  - the attempt prompt renderer.
- Persist fleet state in the `kiln-implementer` store beside the work and attempt ledgers.
- Add `kiln-adapter-herdr`. It drives herdr headless sessions, locally or through saved herdr machines, with bounded child processes. It reports agent states, and placement over its own inventory stays in the adapter.
- Admit an explicit interactive workspace isolation for the `herdr-interactive` provider profile, only for work whose operator policy permits the `interactive_agent` effect. Accept a herdr candidate receipt in candidate materialization, so captured agent commits enter the existing handoff, verification and publication path unchanged.
- Add `kiln-fleet`, one supervised service, with these parts:
  - **`run`:** a tailnet and loopback JSON API with Tailscale-authenticated machine identity, plus the dispatch loop that claims ranked ready work, places it on herdr slots, prompts the agent, renews leases while it works, raises decisions, captures handoffs and stops sessions. It also writes every fleet claim and attempt to the swarm as the only Kiln writer.
  - **`mcp`:** a stdio MCP server for agents.
  - **Client commands:** `claim`, `renew`, `release`, `claims`, `work`, `attempts`, `handoff`, `decisions`, `answer` and `cancel`.
- Add typed Nickel profiles (`config/fleet.ncl`, `config/fleet-inventory.ncl`), flake packages and an operator guide.

## Impact

- New crates: `kiln-adapter-herdr`, `kiln-fleet`. Changed: `kiln-core` (new `fleet` module, swarm overlap, interactive dispatch isolation), `kiln-implementer` (fleet store, herdr receipt capture), `kiln-adapter-swarm` (window, From), the two call sites of the swarm read, `Cargo.toml`, `flake.nix`, docs.
- Lattice dispatch, its receipts and every existing admission rule stay as they are. Interactive work is admitted only by explicit operator policy on the work and an explicit provider profile.
- Verification and publication stay Kiln's existing commands. The fleet service only invokes them when the operator configures them.
- Deployment (onix-core): one `kiln-fleet` service on britton-desktop, herdr headless sessions on the agent machines, and the agents' MCP registration.
