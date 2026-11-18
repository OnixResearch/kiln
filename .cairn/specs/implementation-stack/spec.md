# Implementation Stack Specification

## Purpose

Defines the `implementation-stack` capability.

## Requirements

### Requirement: Opt-in ordered stack publication

r[kiln.implementation.stack] Kiln MUST publish a verified attempt as an ordered stack only when the stack route is explicitly selected. Single pull-request publication MUST stay the default and unchanged; a host without stacked pull requests MUST stay on single pull-request publication; and an Origin destination MUST be refused as unsupported until a separately admitted Origin collaboration adapter exists. A stack plan MUST bind the exact work/version, attempt/fence, handoff and verification, the frozen base and trunk branch, and two to sixteen ordered layers. Each layer MUST bind:

- its exact candidate and tree;
- its parent, which is the base for the bottom layer and the candidate of the layer below otherwise;
- its own admitted verification;
- a deterministic branch, with the branch below as its base.

The top layer MUST be the attempt's verified candidate. Every other layer MUST carry a layer verification admitted under the same canonical CI rules.

#### Scenario: Off by default, unsupported hosts and Origin

r[kiln.implementation.stack.scenario.route]
- GIVEN a verified attempt
- WHEN no stack route is selected, the host has no stacked pull requests, or an Origin destination is requested
- THEN publication MUST use the existing single pull request, or refuse Origin as unsupported, before any effect

#### Scenario: Plan binds every layer exactly

r[kiln.implementation.stack.scenario.plan]
- GIVEN layer candidates, trees, parents, verifications and branches
- WHEN any layer differs from its admitted verification, skips a position, breaks the parent or branch chain, or the top differs from the verified candidate
- THEN the stack plan MUST be refused

### Requirement: Bottom-up stack gates and exact integration

r[kiln.implementation.stack.gate] Each layer MUST keep its own exact-revision verification and landing gates. A layer MAY land only when it and every layer below it are ready or already integrated. A refused or stale lower layer MUST block every higher layer with its refusal, an unknown lower layer MUST leave every higher layer unknown, and no gate MUST be skipped. Integration MUST be admitted only for a contiguous prefix of layers starting at the bottom. Each layer in that prefix MUST be merged with exactly the planned candidate as its second parent and with a landed tree equal to that layer's tree. Its first parent MUST be the frozen base for the bottom layer and the previous layer's merge commit for every layer above. A verified-integrated receipt for the whole stack MUST require every layer.

#### Scenario: A lower layer blocks higher layers

r[kiln.implementation.stack.gate.scenario.lower]
- GIVEN a stack whose lower layer is refused, stale or unknown
- WHEN the layers above it are otherwise ready
- THEN they MUST report that lower refusal or unknown and MUST NOT be ready to land

#### Scenario: No false integration

r[kiln.implementation.stack.gate.scenario.integration]
- GIVEN merged layers whose head, parents or landed tree differ from the plan, or that merged out of order
- WHEN integration is recovered
- THEN no layer or stack receipt MUST be admitted beyond the exact prefix

### Requirement: GitHub stack adapter

r[kiln.github.stack] Before any effect, the GitHub adapter MUST probe the repository's stack endpoints and refuse a repository without stacked pull requests. It MUST then check that each layer candidate has the planned tree and descends from its parent. Only after that MUST it publish each layer's branch and pull request, using the existing exact-branch and pull-request reconciliation. Finally it MUST create, or reconcile once, one GitHub stack whose pull requests, order, heads and trunk are exactly the plan's. Re-entry MUST reuse the same branches, pull requests and stack. A different existing stack MUST be refused, and an unresolved stack creation MUST stay unknown. Merge recovery MUST read each layer from the bottom and return a receipt only under the exact stacked merge mapping. It MUST NOT merge, rebase, retarget or unstack.

#### Scenario: Frozen two-layer stack and re-entry

r[kiln.github.stack.scenario.publish]
- GIVEN a verified two-layer stack plan and a repository with stacked pull requests
- WHEN it is published and then published again
- THEN both branches, both pull requests and one stack MUST exist exactly once, and the second run MUST issue no mutation

#### Scenario: Unsupported, conflicting or uncertain remote state

r[kiln.github.stack.scenario.refused]
- GIVEN a repository without stacks, an existing stack with other members, or a lost stack creation response
- WHEN publication runs
- THEN it MUST refuse before any effect, refuse without changing the stack, or reconcile once and otherwise stay unknown
