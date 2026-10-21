# Tasks

## Phase 1: Scope and admission

- [x] [serial] Inspect observer/status/readback dependencies; confirm the reverse tailnet path and the pinned remote coordinator/profile prerequisites. r[kiln.radicle_ci.distinct_status.authority] r[kiln.radicle_ci.distinct_status.transport]
- [x] [serial] Record the actual interfaces and admit proposal, design and tasks before implementation. r[kiln.radicle_ci.distinct_status.authority]

## Phase 2: Remote observer implementation

- [x] [serial] Extend the typed distinct-host profile with the local CI tailnet address and explicit remote observer placement, with positive and negative fixtures. r[kiln.radicle_ci.distinct_status.authority]
- [x] [serial] Extend the remote shell for the observer home/process and the pinned remote `status-reconcile` read; keep publisher placement unchanged. r[kiln.radicle_ci.distinct_status.transport]
- [x] [serial] Restart the local CI node on the tailnet address, move the observer home remote, connect and seed it, and keep status creation/signing local. r[kiln.radicle_ci.distinct_status.transport]
- [x] [serial] Replace the observer readback with remote storage reduction and exact equality; require pre-sign absence. r[kiln.radicle_ci.distinct_status.readback]

## Phase 3: Distinct-host status proof

- [x] [serial] Exercise exact remote readback for success and genuine failure, and retain no status/object for the unavailable provider. r[kiln.radicle_ci.distinct_status.readback] r[kiln.radicle_ci.distinct_status.evidence]
- [x] [serial] Retain remote observer logs, commands, readbacks, connection lines and remote retirement; keep duplicate/restart evidence. r[kiln.radicle_ci.distinct_status.evidence] r[kiln.radicle_ci.distinct_status.claims]

## Phase 4: Delivery

- [x] [serial] Run focused tests and every required repository gate; synchronize and archive only after the distinct-host status prerequisites and acceptance criteria actually pass. r[kiln.radicle_ci.distinct_status.evidence]
