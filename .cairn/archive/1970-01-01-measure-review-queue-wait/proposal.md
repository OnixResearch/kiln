## Why

Issue [#61](https://github.com/OnixResearch/kiln/issues/61), in its [17 November comment](https://github.com/OnixResearch/kiln/issues/61), asks Kiln to measure review-queue wait under a synthetic parallel-agent load before it changes any admission, budget or backpressure policy in response to review congestion. The review queue in `code_review/queue.rs` builds and tests exact integration candidates and submits them serially or inside a speculative window. Kiln has no retained trace of how long candidates wait there when many arrive at once. Closed #88 measured build-thread contention, which does not answer this question.

## What Changes

- A typed Nickel load shape, `kiln.review-queue-wait.load.v1`, declares a finite synthetic clock, the queue bound, the queue modes and a frozen set of candidates. Each candidate has an arrival tick, a CI result (pass, fail or unknown) with its duration, and an optional submitter deadline. A committed JSON export is checked for freshness, and a negative fixture must be refused.
- A pure core in `kiln-web-service` admits the load, classifies each observed candidate into one named outcome, summarizes the waits and builds a `kiln.review-queue-wait.receipt.v1` receipt. The receipt has four separately typed series: review-queue wait, merge-queue wait, ready-work admission wait and inference backpressure. The core refuses a receipt that relabels a series or whose summary does not match its candidates.
- An off-by-default example, `review-queue-wait`, runs the load through the review queue's real commands, Git provider, SQLite store and engine CI slot, with a scripted workflow provider and a synthetic clock. It runs serial and speculative modes and writes the receipt. An integration test runs the frozen load and compares the outcome table with a frozen expectation.
- `docs/fair-admission.md` documents the fixture, the outcome definitions and the series boundaries. The working paper and the X posts cited on the issue appear there only as context.

## Impact

Consumers: maintainers deciding on admission or backpressure changes for #61. Owners: Kiln review queue and admission maintainers. Files: Nickel contract and fixtures, the committed export and expectation, the Nickel fixture gate script, the new core module, example and test, the guide and the changelog. No service, default policy or stored state changes.

## Non-goals

This change does not run a production load test, and it measures no GitHub or other external merge queue. It adds no new fairness algorithm or latency SLO, and it does not reuse #88's build-contention numbers. It also does not change admission, queue or budget policy; any such change needs its own design citing this measurement.
