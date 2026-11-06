## Phase 1: Core

- [x] [serial] Add the optional `supersede_queued` trigger field and retain the requested reference on runs. r[kiln.web.supersede.policy] r[kiln.web.supersede.identity]
- [x] [serial] Cancel older queued runs through durable queue control and active runs through exact provider control. r[kiln.web.supersede.cancel] r[kiln.web.supersede.active]

## Phase 2: Verification

- [x] [serial] Add positive and negative automation coverage, keep existing dispatch tests passing, and document the opt-in. r[kiln.web.supersede.policy]
- [x] [serial] Run focused and full web-service tests, Clippy, rustfmt and Cairn validation. r[kiln.web.supersede.cancel]

## Phase 3: Origin reconciliation

- [x] [serial] Reproduce shared-ID trigger/schedule origin collision for retained active and queued schedules after restart, and protect the reverse schedule-arrival direction. r[kiln.web.supersede.identity] r[kiln.web.supersede.identity.scenario.origin]
- [x] [serial] Select candidates through retained trigger-kind source provenance and exact run linkage without changing serialized metadata or admitted registration namespaces. r[kiln.web.supersede.identity] r[kiln.web.supersede.identity.scenario.origin]
- [x] [serial] Verify positive trigger supersession, manual/reference/pull-request exclusions, restart behavior and complete source gates before archive. r[kiln.web.supersede.identity] r[kiln.web.supersede.active]


- [x] [serial] Reproduce the durable Start-request/missing-source-link crash window and exact fingerprint mismatch; prove a later replay cannot cancel the newer successor. r[kiln.web.supersede.identity] r[kiln.web.supersede.identity.scenario.origin]
- [x] [serial] Recover only exact typed Start reservations, preserve the linked fast path and restrict cancellation to canonically older run ordinals without new fields or effects. r[kiln.web.supersede.identity] r[kiln.web.supersede.cancel]

