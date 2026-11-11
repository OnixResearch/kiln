## Phase 1: Transport

- [x] [serial] Add the fixed-origin authenticated polling client with bounded response and deadlines. r[kiln.github.poll.transport]
- [x] [serial] Filter pull requests and validate record timestamps, actors and states. r[kiln.github.poll.transport.scenario.pages]
- [x] [serial] Parse at most one strictly advancing bounded Link page. r[kiln.github.poll.transport.scenario.bounds]

## Phase 2: Failure boundaries

- [x] [serial] Distinguish authorization, not-found, rate-limit, transport and payload failures. r[kiln.github.poll.transport.scenario.refused]
- [x] [serial] Keep scheduling, admission, dispatch and retries outside the transport. r[kiln.github.poll.transport]
