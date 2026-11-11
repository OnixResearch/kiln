## Phase 1: Queue

- [x] [serial] Add the bounded durable delivery queue keyed by delivery identity. r[kiln.implementation.delivery_queue]
- [x] [serial] Add optional implementation queue configuration to the GitHub profile and Nickel contracts. r[kiln.implementation.delivery_queue]

## Phase 2: Webhook integration and verification

- [x] [serial] Authenticate issues deliveries and enqueue exact bytes; keep non-issue and absent-queue behavior unchanged. r[kiln.implementation.delivery_queue]
- [x] [serial] Test idempotent queueing, forged refusal, non-issue pass-through and queue bounds. r[kiln.implementation.delivery_queue.scenario.idempotent] r[kiln.implementation.delivery_queue.scenario.forged]
