# Lattice runtime pin and agent orchestration

## Decision

Kiln imports Lattice `85a38b6a4bceeae4bb2057cf157e21752100f1b5` for both CI and agent jobs.

The imported `lattice-workflow-contract` and `lattice-workflow-transport` crates are `AGPL-3.0-or-later`. The operator accepted that license. Kiln's previous gate required `MIT` and stopped at `70496e67`. That gate is retired for this import. `LICENSE-MIT-HISTORICAL` in the import records earlier grants and does not license the new bytes. Kiln-owned adapter source stays `MIT OR Apache-2.0`. This record does not certify combined-work license compatibility.

There is no root `cargo deny` license allow-list. The gate is the pinned license string in `kiln-adapter-lattice`, the Nickel profiles, and `scripts/sync-lattice-workflow-fixtures.rs`.

## Handler

`85a38b6a` is the contract import and the handler operators run. It contains `shell_success` (`6339864d`) and observed cancellation (`813076d7`). Request schema `lattice.workflow-request.v2` and observation schema `lattice.workflow-observation.v1` are unchanged.

`70496e67` is the previous MIT import. `c513d94d` is historical Aspen evidence. Neither is the CI handler.

The admitted capability cohort is cancel only. Pause and resume fail before transport. The Lattice `workflow-exchange serve` handler still rejects those actions.

## Agent jobs

Kiln admits an agent review job in `kiln-core`: one Lattice target (`pi`, `clankers`, or `animus`), a reviewed prompt identity, a budget inside the named ceilings (1 invocation, 32,768 tokens, 900,000 ms), and a permission set that must include `agent/execute` and cannot exceed `read/candidate` and `write/review-note`. The operation identity commits those facts. A wider permission, a larger budget, or a different vote policy is a different graph and needs a new reviewed Lattice route.

The agent cohort is separate from CI. Its node kind is `agent`, its effect class is `custom`, and its adapter kind is `agent`. Dispatch uses the existing workflow exchange. Lattice's agent runtime, budgets, approvals, and permission check execute the job. Kiln does not call a model.

Vote policy stays in Kiln. A completed Lattice observation does not count as an approval, including when policy is binding, and it is not a Patchgate receipt. Isolation, untrusted-PR holds, journals, and billing stay in Kiln.

## What this slice does not do

It does not vendor the Lattice application, start a Lattice process, or prove a live agent route. The exchange contract has no budget or permission fields; those are bound only by the operation identity to a route an operator reviews in Lattice. Source binding still injects environment variables into shell nodes only, so an agent node does not yet receive `LATTICE_SOURCE_*` through the handler binder.

The standalone Aspen host still links published Kiln `69afc269` and Lattice `70496e67`. Moving that lock to `85a38b6a` requires private Basalt `d913dc01e765c9b297df5fcc57dfa06aac39bc74`. GitHub returns repository not found for `ssh://git@github.com/OnixResearch/basalt.git`. Republishing the host against this Kiln revision is a later cohort.

## Proposed Lattice follow-ups

These are not changes to the Lattice repository.

1. Refuse to serve a handler profile that sets `controls.pause` or `controls.resume`, matching Kiln's cancel-only cohort. Serving a profile that advertises them still accepts the socket and rejects the control later. See Lattice issue 2.
2. Publish a consumer route record for an agent review graph: node kind `agent`, effect class `custom`, adapter kind `agent`, ability `agent/execute`, and the budget ceiling above, so operators are not left to invent the route. See Lattice issue 3 for the related runtime record; Kiln now names `85a38b6a` itself.
3. Add a source-binding mode that supplies the admitted provider, repository, and revision to an agent node's context without granting the agent a way to widen its permissions. `shell_environment` only patches shell nodes.
