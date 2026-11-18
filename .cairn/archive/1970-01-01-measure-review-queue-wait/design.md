## Context

`code_review/queue.rs` keeps FIFO entries per target. Serial mode advances only each target's head: it builds an exact merge candidate (`preparing`, then `waiting_ci`), starts canonical CI for that commit, and after a successful terminal CI it runs fresh admission and an exact-target CAS submit (`submitting`, then `submitted`). Failures eject the entry (`blocked`), a reserved provider operation leaves it `uncertain`, and `dequeue` cancels it. Speculative mode stacks candidates inside an adaptive window and rebuilds later entries when an earlier one is ejected or withdrawn. The engine dispatches one CI execution per project at a time, and a run whose outcome is unknown keeps that slot. `GET /api/code-review` returns every queue entry, and `GET /api/runs/<id>` returns a run's phase and outcome.

## Goals and Success Contract

A maintainer can run one frozen synthetic load through the real queue path and get a receipt. For each candidate, the receipt gives the exact source and candidate identities, when the candidate arrived, was enqueued, started construction, requested CI, was dispatched to the workflow provider and reached a terminal state, plus one named outcome. A number cannot appear under the wrong series, and the receipt says plainly what it did not measure.

## Decisions

### Decision: synthetic clock, observed at tick boundaries

**Choice:** the fixture owns the clock. Tick `t` is at `T0 + t * tick_ms`, and every command and `advance_queue` call in that tick uses that time. In each tick the fixture does the following, in order:
1. Enqueues the tick's arrivals.
2. Withdraws entries whose submitter deadline has passed.
3. Applies due CI results.
4. Polls the engine.
5. Advances the queue.
6. Reads the queue snapshot and the CI runs.

Every recorded time is the first tick at which the fixture observed that state, in milliseconds since tick 0. The resolution is one tick.

**Rationale:** the engine's own run timestamps use the host clock, and mixing them with the queue's request time would make waits meaningless. Observing at tick boundaries through the public API needs no new hook in the queue.

### Decision: outcomes are derived from recorded signals only

**Choice:** one pure classifier, applied in this order:
- An enqueue refusal is `rejected` with the refusal code.
- `submitted` is `submitted`.
- `cancelled` after the fixture's deadline withdrawal is `expired`, and any other cancellation is `cancelled`.
- `blocked` with `review_queue_ci_failed` is `failed`. Any other `blocked` is `rejected` with the recorded detail.
- `uncertain` is `unknown`.
- An entry still active at the horizon is `unknown` when its current CI run's outcome is unknown, and `waiting` otherwise.
- Any status outside the queue's closed set is `unknown`.

**Rationale:** the review queue has no entry deadline. So expiry is defined only by the load: the synthetic submitter withdraws its entry with the ordinary `dequeue` command at its declared deadline. The receipt names this definition rather than inventing a queue deadline.

### Decision: four typed series, three not measured here

**Choice:** the receipt has fixed slots: `review_queue_wait`, `merge_queue_wait`, `ready_work_admission_wait` and `inference_backpressure`. Each slot carries its own `series` label, either measured data or a not-measured reason. The last three slots use an uninhabited value type, so this fixture cannot place numbers there. On read, the core refuses a label that does not match its slot, an unknown field, a load identity that does not match the embedded load, and an arm summary that is not exactly recomputed from that arm's candidates.

**Rationale:** the issue requires that a number from one series can never be labelled as another. Rust types prevent it when a receipt is built, and read-back validation catches edited receipts.

### Decision: load in Nickel, export checked, core re-admits

**Choice:** `config/review-queue-wait-contracts.ncl` defines the `Load` contract and the frozen example. `config/fixtures/review-queue-wait-positive.ncl` exports it to `fixtures/review-queue-wait/load.json`, and `scripts/check-nickel-fixtures.sh` refuses a stale export. A negative fixture must fail to export. The Rust core admits the JSON again with the same named bounds, and the load identity is a domain-tagged BLAKE3 hash of the canonical typed load.

### Decision: an example for runs, a test for regression

**Choice:** `examples/review-queue-wait.rs` (with `--load` and `--out`) and `tests/review_queue_wait.rs` share one harness module by path. The harness builds a fresh bare Git repository with one source commit per candidate, each touching only its own file. It also builds review and service profiles: one CI slot, the load's queue bound, and the speculative capability for the speculative arm. The scripted workflow provider reports each dispatched execution's declared result `ci_ticks` after dispatch. The test compares each arm's outcome and time table with `fixtures/review-queue-wait/expected.json` and checks that the receipt reads back.

## Risks / Trade-offs

- A legitimate change to queue behaviour changes the frozen table. The test then fails until the expectation is refreshed after review, as intended for a frozen measurement.
- Synthetic CI durations and one CI slot are part of the load's meaning. The receipt measures queue mechanics under that declared shape, not production CI or reviewer latency.
- Tick resolution hides sub-tick ordering. Within a tick, events follow the documented step order.

## Verification Strategy

Unit tests cover:
- load admission bounds and negatives;
- every classifier branch;
- nearest-rank summaries, including empty ones;
- receipt read-back refusals for a relabelled series, an edited summary, a mismatched load identity and unknown fields.

The integration test runs the frozen load in both modes and compares the outcome table. The Nickel gate script checks the export and the negative fixture. Clippy, scoped formatting and the Cairn gates and validation also run. One example run is retained as evidence.
