## ADDED Requirements

### Requirement: Speculative queue window projection in code-review API

r[kiln.review.queue.speculative.window_projection]
`GET /api/code-review` MUST include the recorded `queue_windows` and active `WindowPolicy` bounds (`floor`, `ceiling`, `increase`, `decrease`, current depth, and per-target mode) alongside `state.queue` so the console can display exact speculative merge train depth and policy without hidden client inference.

#### Scenario: Code-review API returns active speculative window bounds

r[kiln.review.queue.speculative.window_projection.scenario.exposed]
- GIVEN a code-review target configured with a speculative `WindowPolicy`
- WHEN a client fetches `GET /api/code-review`
- THEN the response MUST serialize `queue_windows` with the exact target ref, mode, active window depth, and configured bounds
- AND targets in `serial` mode MUST omit speculative window claims rather than fabricating defaults
