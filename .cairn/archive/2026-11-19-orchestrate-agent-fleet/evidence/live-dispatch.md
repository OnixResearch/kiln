# Live dispatch, 2026-11-19

onix-core deployed `kiln-fleet` as follows:

- **Coordinator:** britton-desktop runs `kiln-fleet.service` as the system account `kiln-fleet`.
- **Agent hosts:** britton-desktop and aspen3 each run a headless herdr 0.9.3 session, `kiln`. On aspen3 it is reached through the saved herdr machine `aspen3-kiln`.
- **Clients:** every NixOS swarm member has the `kiln-fleet` client and its MCP registration.

The configuration passed `kiln-fleet check-config`.

## Dispatch through capture

| Step | Observation |
| --- | --- |
| Admission | Cairn change `greet-by-name` in `brittonr/kiln-fleet-sandbox`, admitted with `kiln-cairn-intake` under a policy that permits `write_source` and `interactive_agent`; base `6740c5422f4d3101a720ca74f76a4b2e3e70018d`; verification profile `a329093c818b54eb8eee9975421285b595aaa8592c25243d99c3da89df45d55e`. Work `3603cd1706764b4ed9c27c2652d57242724d18771379d5f26b0d95108d333c28`. |
| Claim and placement | `dispatch.claimed` gave attempt `bef74ee6e0490bb9e6d6de24ff651aca6ae902b67e81e469a3f8b10e1d9a4d9c`, fence 1, on aspen3. Kiln posted `CLAIM brittonr/kiln-fleet-sandbox greet.sh` from its own swarm account (`mirror.post` accepted). `dispatch.placed` started an OMP agent in a herdr worktree on branch `kiln/impl-3603cd170676-bef74ee6e049`. |
| Agent | OMP changed only `greet.sh`, ran `sh check.sh` and committed `dd104e7b96e8c3f34811ba2de2e02f552cdbd2f1`. It then called the `kiln_handoff` MCP tool and received 202. |
| Handoff | `api.handoff`: the API named the caller `aspen3` through `tailscale whois`. |
| Capture | Kiln fetched the branch over ssh with its restricted key into `refs/kiln/fleet-handoffs/<attempt>`, checked that the commit descends from the base, and exported the tree with exact modes (755 `greet.sh` and `check.sh`, 644 the rest) under the service's umask of 077. `materialize_candidate` admitted the `kiln.herdr.candidate-receipt.v1` receipt: `changed_paths` is `["greet.sh"]`, the candidate tree is `a48934dfcd61ffa3f2ab358a69af8d5433cfb8a9`, and `refs/kiln/implementation-candidates/<attempt>` is `8bde909678bb1939faf73e593045e343b9f7efdc`. The attempt phase became `candidate_produced`. |

## Claims from agents

- Another agent on aspen3 (`omp`, session `01a1eff8`) claimed `brittonr/forecast .` and `brittonr/forecast-kernel .` through the client. Kiln mirrored both claims as `kiln on britton-desktop for omp on aspen3`.
- A claim for `brittonr/forecast src` from britton-desktop was refused with 409 `fleet_claim_conflict`, naming `omp on aspen3, session 01a1eff8`.

## Defects found and fixed during rollout

- **Handoff fetch on the coordinator.** The coordinator fetched its own machine's handoffs from the worktree path, which its account cannot read. It now fetches through `ssh_target` on every machine (`81b1b50a`).
- **Export modes.** The export took its file modes from the service umask (077), and materialization refuses 0600 files. `git archive` now runs with `tar.umask=0022` and tar extracts with `-p` (`81b1b50a`).
- **Lease expiry after capture.** Captured sessions stopped renewing their attempt. A candidate awaiting verification would therefore expire as `lease_expired`, and the work would be dispatched again. Captured attempts now keep their lease (`7210e049`).
- **onix-core configuration.** The iroh-news children needed `HOME` in the swarm `environment`, because Kiln clears their environment.

## Not shown

- No verification ran: `verify_argv` and `publish_argv` are unset.
- No agent's honesty was proven.
- The second slot on each host and a britton-desktop placement were not exercised live.
