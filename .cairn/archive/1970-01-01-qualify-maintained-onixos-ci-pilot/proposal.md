# Qualify the maintained runtime with a real OnixOS CI consumer

## Why

The real Onix shadow proves the historical published `8c9338e5c10a0e16ee3042d11583ccccf6efe7e9` executable cohort, not the maintained standalone host whose Kiln dependencies are pinned to `69afc2697f4330005797256135ae81b2722a125c`. The Onix evaluator currently admits only the discovery-only seed/report fixture. Observation and UCAN authentication exist, but the Aspen operation ledger is not a canonical CI event journal or an observation source.

The current consumer is OnixOS's existing standalone home-activation application, generated from its real inventory and Nickel service module. The target is a bounded source-bound application build and CI observation, not a whole system image, activation of an operator home, or production deployment.

## What Changes

- Bind each Onix discovery profile to one explicit immutable source, flake-byte identity, selector and platform, independently of the fixed evaluator/plugin cohort. Migrate every constructor caller; preserve stock selection and rejection behavior. r[kiln.onixos_pilot.source]
- Add an isolated pilot shell that authenticates an operator-signed request before effects, uses the existing versioned operator Aspen ingress, and names genuine published Radicle source facts. The separately pinned OnixOS consumer is an explicit workflow input. Do not fabricate a forge delivery or represent the discovery fixture as the real build. r[kiln.onixos_pilot.trigger]
- Execute the maintained host/extension, real Lattice workflow and exact rooted Nix build provider; retain their complete source, executable, profile and input bindings. r[kiln.onixos_pilot.runtime]
- Persist canonical CI commands/events before dispatch and after admitted completion; rebuild read-only observation from that journal. r[kiln.onixos_pilot.journal]
- Exercise duplicate delivery, host restart/reconciliation, genuine negative outcomes, rooted output retention and UCAN-authenticated HTTPS observation. r[kiln.onixos_pilot.recovery] r[kiln.onixos_pilot.observation] r[kiln.onixos_pilot.failure]
- Retain repeatable commands and exact cohort-bound evidence; keep production state, status writes and cutover disabled. r[kiln.onixos_pilot.evidence]

## Impact

- **Files**: `crates/kiln-adapter-nix-eval`, its existing caller/tests and documentation; a dedicated pilot composition shell under `crates/`; typed Nickel pilot inputs under `config/`; the pilot's bounded operator runner under `scripts/`; workspace/package and license declarations; native Cairn requirements and receipts.
- **Testing**: existing discovery baseline; positive/negative source-binding and durable transition regressions; actual non-fixture OnixOS discovery/build; maintained Aspen/Lattice restart and duplicate observations; real TLS/UCAN requests and denials; workspace tests, strict Clippy, formatting, no-std, Nickel, Octet and relevant Nix gates. Historical and cached observations remain separately labeled.
- **Owner and adoption**: Kiln maintainers own the source admission, composition and repeatability rail. OnixOS owns the inventory/service semantics. Operators explicitly select the pilot inputs and retain all deployment, credential, status and publication authority.

## Non-goals

No new forge, scheduler, provider fallback, production service, remote status mutation, source-trust claim, sandbox claim, activation/deployment, distributed exactly-once claim or release approval. The main cutover of completed host-closeout revision `6c6e4f220fe0234ce607085fa622b580fd1cd016` is a separately authorized prerequisite, not qualification of newly built pilot executables.
