## Phase 1: Contracts

- [x] [serial] Define and gate the GitHub profile, authenticated event, replay, workflow, exact-source, publication and non-claim contracts before implementation. r[kiln.github.profile] r[kiln.github.authentication] r[kiln.github.nonclaims]

## Phase 2: Provider shell

- [x] [parallel] Implement pure bounded push/PR admission and positive/negative signature, repository, action and fork-policy fixtures. r[kiln.github.authentication] r[kiln.github.events]
- [x] [parallel] Implement bounded SecretSpec resolution and exact GitHub status planning, HTTPS publication and ambiguous-write reconciliation. r[kiln.github.status] r[kiln.github.secrets]
- [x] [parallel] Implement the exact-source job executable with identity-bound configuration, Git snapshot admission and bounded command execution. r[kiln.github.source]
- [x] [serial] Integrate bounded HTTP acknowledgement, durable replay/dispatch reservations, admitted Lattice observations, restart recovery and status state. r[kiln.github.replay] r[kiln.github.workflow] r[kiln.github.shell]
- [x] [serial] Add typed Nickel contracts, executable packaging and operator instructions without changing existing provider behavior. r[kiln.github.profile] r[kiln.github.nonclaims]

## Phase 3: Evidence

- [x] [serial] Exercise signed push and PR intake through actual source/job execution and result publication, including duplicate/restart, malformed, stale, timeout, cancellation, unknown and provider-failure cases. r[kiln.github.events] r[kiln.github.replay] r[kiln.github.source] r[kiln.github.workflow] r[kiln.github.status]
- [x] [serial] Run focused adapter and required workspace, policy, traceability, Nickel, Nix and Cairn checks; retain scoped evidence and explicit live-service prerequisites. r[kiln.github.shell] r[kiln.github.nonclaims]
