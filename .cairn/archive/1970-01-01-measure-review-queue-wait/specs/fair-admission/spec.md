# Fair admission delta

## ADDED Requirements

### Requirement: Synthetic review-queue wait measurement

r[kiln.admission.queue_wait] Kiln MUST keep an off-by-default synthetic fixture that runs a frozen, typed load of exact source candidates through the review queue's ordinary enqueue and dequeue commands and advance loop, under a declared finite load shape and a synthetic clock, in both serial and speculative queue modes. Its receipt MUST record, for every candidate, the exact source commit, every exact integration candidate and CI run, the arrival, enqueue, start, CI request, CI dispatch and terminal times observed at tick boundaries, and exactly one named outcome: submitted, waiting, rejected, expired, failed, unknown or cancelled. Review-queue wait, merge-queue wait, ready-work admission wait and inference backpressure MUST be separate typed series. A series the fixture did not exercise MUST be reported as not measured, with its reason. A receipt MUST be refused when a series is labelled as another series or when its summary does not match its candidates. The fixture MUST NOT change default admission, queue or budget policy, and findings from outside Kiln MUST NOT be used as pass or fail thresholds.

#### Scenario: Frozen load records every outcome

r[kiln.admission.queue_wait.scenario.outcomes]
- GIVEN the frozen load with passing, failing and unknown CI, a submitter deadline and an arrival beyond the queue bound
- WHEN the fixture runs it in each queue mode
- THEN every candidate MUST carry its exact identities, observed times and one named outcome. A candidate that never reached a terminal state MUST be reported as waiting or unknown, never as submitted

#### Scenario: Series stay separate

r[kiln.admission.queue_wait.scenario.series]
- GIVEN a receipt from a fixture that has no external merge queue, ready-work admission or inference backpressure in its path
- WHEN the receipt is read back
- THEN only review-queue wait MUST carry numbers, the other three series MUST be not measured with a reason, and a receipt that relabels a series or whose summary differs from its candidates MUST be refused

#### Scenario: Malformed load is refused

r[kiln.admission.queue_wait.scenario.load]
- GIVEN a load whose schema, bounds, candidate identities, modes or windows are invalid
- WHEN it is exported from Nickel or admitted by the fixture
- THEN it MUST be refused before any queue command runs
