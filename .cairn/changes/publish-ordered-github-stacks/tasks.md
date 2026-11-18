## Phase 1: Contract

- [x] [serial] Specify the route, layer verification, plan, gate, integration and adapter behaviour in the proposal, design and delta spec. r[kiln.implementation.stack] r[kiln.implementation.stack.gate] r[kiln.github.stack]

## Phase 2: Core

- [x] [serial] Add the route, layer verification on shared CI rules, plan admission, bottom-up gate and prefix integration, with positive and negative unit tests. r[kiln.implementation.stack.scenario.route] r[kiln.implementation.stack.scenario.plan] r[kiln.implementation.stack.gate.scenario.lower] r[kiln.implementation.stack.gate.scenario.integration]

## Phase 3: GitHub adapter

- [x] [serial] Add the stack probe, ancestry checks, per-layer publication, exact stack creation and reconciliation, and read-only stacked merge recovery, with loopback fixtures for publish, re-entry and refusals. r[kiln.github.stack.scenario.publish] r[kiln.github.stack.scenario.refused] r[kiln.implementation.stack.gate.scenario.integration]

## Phase 4: Documentation and evidence

- [x] [serial] Document GitHub's stack and merge behaviour and Kiln's guards, update the changelog, run the focused checks and record evidence. r[kiln.implementation.stack] r[kiln.github.stack]

Verification is recorded in `evidence/verification.md`. Single pull-request
publication is unchanged, the stack route is not selectable from the operator
CLI, and no live GitHub stack was published, merged or recovered.
