## Phase 1: Core

- [x] [serial] Add the optional `supersede_queued` trigger field and retain the requested reference on runs. r[kiln.web.supersede.policy] r[kiln.web.supersede.identity]
- [x] [serial] Cancel older queued runs through durable queue control and active runs through exact provider control. r[kiln.web.supersede.cancel] r[kiln.web.supersede.active]

## Phase 2: Verification

- [x] [serial] Add positive and negative automation coverage, keep existing dispatch tests passing, and document the opt-in. r[kiln.web.supersede.policy]
- [x] [serial] Run focused and full web-service tests, Clippy, rustfmt and Cairn validation. r[kiln.web.supersede.cancel]
