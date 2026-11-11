## Phase 1: Loop

- [x] [serial] Add the repeated drain mode with positive interval, optional iteration bound and signal shutdown. r[kiln.implementation.drain_loop]
- [x] [serial] Reuse the existing drain inputs and stop on the first error. r[kiln.implementation.drain_loop]

## Phase 2: Verification

- [x] [serial] Test the bounded loop over an empty queue with CLI drain/status regression. r[kiln.implementation.drain_loop.scenario.bounded]
