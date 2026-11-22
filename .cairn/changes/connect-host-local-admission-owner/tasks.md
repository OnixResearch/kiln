## Phase 1: Host-local owner core

- [x] [serial] Add `HostOwnerProfile` with Rust re-admission, the typed Nickel contract, positive and negative fixtures and the committed export checked for staleness. r[kiln.admission.host_owner_profile] r[kiln.admission.host_owner_profile.scenario.exports]
- [x] [serial] [depends:kiln.admission.host_owner_profile] Derive policy, account, queue rows, usage and eligibility observations, plan reservations, decide owner replies and plan observed releases as pure functions. r[kiln.admission.host_owner]

## Phase 2: Shell and operator commands

- [x] [serial] [depends:kiln.admission.host_owner] Add the implementer shell and `host-owner-admit|observe|reserve|release` commands over the existing admission transitions and owner port. r[kiln.admission.host_owner.scenario.reserve]
- [x] [serial] Route the operator `claim` through admission when a policy is configured. r[kiln.admission.operator_claim] r[kiln.admission.operator_claim.scenario.governed]

## Phase 3: Intake selection and packaging

- [x] [serial] Add explicit issue/label selection to the intake policy, webhook drain, polling and the Nickel contract. r[kiln.implementation.intake_selection] r[kiln.implementation.intake_selection.scenario.selected] r[kiln.implementation.intake_selection.scenario.refused]
- [x] [serial] Promote the canonical CI graph example to the installed `implementation-verification-graph` command. r[kiln.implementation.verification_graph_cli] r[kiln.implementation.verification_graph_cli.scenario.constructed] r[kiln.implementation.verification_graph_cli.scenario.refused]

## Phase 4: Verification

- [x] [serial] [depends:kiln.admission.host_owner.scenario.reserve] Add durable refusal and restart tests: unregistered repository, stale observation, over capacity, duplicate reservation, replay and observed release. r[kiln.admission.host_owner.scenario.refused] r[kiln.admission.host_owner.scenario.replay]
- [x] [serial] Update docs and changelog and run the repository gates without live claims. r[kiln.admission.host_owner]
