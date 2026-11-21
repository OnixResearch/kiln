## Phase 1: Core settlement

- [x] [serial] Add `AttemptLedger::settle_dispatch_without_execution` and its `Implementer` shell method: a current attempt with a reservation, a transport intent and no recorded workflow ends as an infrastructure failure with a bounded cause and no transport intent; test refusal without intent, after a recorded workflow, at a stale fence, across restart and that claiming is not blocked by an unknown outcome. r[kiln.implementation.lattice_cli.scenario.refusal]

## Phase 2: Worker transport

- [x] [serial] [depends:kiln.implementation.lattice_cli.scenario.refusal] Add the pure refusal and observation classifier in `kiln-adapter-lattice` with positive and negative cases for every reason in Lattice's refusal table, crossed records and malformed output, and wire settlement (only a non-zero exit is classified) and the closed-before-execution observation into `worker::run`. r[kiln.implementation.lattice_cli.scenario.refusal] r[kiln.implementation.lattice_cli.scenario.closed]
- [x] [serial] [depends:kiln.implementation.lattice_cli.scenario.closed] Add the pure credential-variable decision and the launch environment, refusing before transport intent; test missing, malformed, malformed-key, non-`env`, reserved and undeclared cases, and the delivered, withheld and undeclared environments through a real child process. r[kiln.implementation.lattice_cli.scenario.credential]
- [x] [serial] [depends:kiln.implementation.lattice_cli.scenario.credential] Require `request.dispatch_plan == reservation.plan` before transport intent and the echoed plan on observations; prepare campaign requests twice to bind `dispatch_plan`, and check it on retained requests. r[kiln.implementation.lattice_cli.scenario.plan]

## Phase 3: Configuration, fixture and documentation

- [x] [serial] [depends:kiln.implementation.lattice_cli.scenario.plan] Admit only transport v2 in `config/implementation-worker.ncl` with positive and negative fixtures. r[kiln.implementation.lattice_cli.scenario.config]
- [x] [serial] [depends:kiln.implementation.lattice_cli.scenario.config] Move `kiln-implementation-fixture bind-execution` and `docs/implementation-runbook.md` to request v3, transport v2, `network:false` and the brokered route, and record the change in `CHANGELOG.md`. r[kiln.implementation.lattice_cli]

## Phase 4: Verification

- [x] [serial] [depends:kiln.implementation.lattice_cli] Run the repository gates and a composed local requalification against the Lattice `db64cd65` binary with pinned OMP, and retain the evidence. r[kiln.implementation.lattice_cli]

## Phase 5: Review amendments

- [x] [serial] [depends:kiln.implementation.lattice_cli] Retain `stale_fence` refusals, settle refusals only from the first `run`, and strictly deserialize refusal and closure records (`deny_unknown_fields`, duplicate keys refused, `revision_refusal` only for its reasons); test every table reason from `run` and `recover`, duplicates, unknown fields and refusal-plus-result. r[kiln.implementation.lattice_cli.scenario.refusal] r[kiln.implementation.lattice_cli.scenario.closed]
- [x] [serial] [depends:kiln.implementation.lattice_cli.scenario.refusal] Bind the Lattice program and state root at the first launch before its transport intent and recover only with the bound values; test a changed profile, an absent binding, a missing or symlinked bound root, a conflicting rebinding and the 0400 mode. r[kiln.implementation.lattice_cli.scenario.bound_root]
- [x] [serial] [depends:kiln.implementation.lattice_cli.scenario.bound_root] Resolve the credential environment right after claim authorization and before plan reservation; test that missing and reserved credentials leave no reservation. r[kiln.implementation.lattice_cli.scenario.credential]
- [x] [serial] [depends:kiln.implementation.lattice_cli.scenario.credential] Rerun the composed qualification with a moved-root scenario, and amend the proposal, design, spec delta, synced spec, runbook and changelog. r[kiln.implementation.lattice_cli]
