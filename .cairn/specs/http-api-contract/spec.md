# Http Api Contract Specification

## Purpose

Defines the `http-api-contract` capability.

## Requirements

### Requirement: Bounded HTTP API contract

r[kiln.web.http_contract] The web service MUST expose a versioned, GET-only
OpenAPI 3.0.x contract for run summary and execution-binding/artifact metadata.
Authentication failures, transport failures and schema mismatches MUST remain
distinct. Unknown outcomes and lost observation MUST be preserved; HTTP 2xx or
successful deserialization is not execution or CI evidence.

#### Scenario: Authenticated project list

- GIVEN an operator role
- WHEN GET /api/projects is called
- THEN the response is 200 with available and enrolled arrays.

#### Scenario: Unauthorized worker scope

- GIVEN a worker role accessing /api/projects
- WHEN the request is made
- THEN the response is 403 with code worker_scope.

#### Scenario: Missing run

- GIVEN an unknown run id
- WHEN GET /api/runs/{id} is called
- THEN the response is 404 with code run_missing.

#### Scenario: Invalid pagination

- GIVEN an out-of-range limit
- WHEN GET /api/runs is called
- THEN the response is 400 with code page_bound.

#### Scenario: Execution binding scope

- GIVEN a viewer role accessing execution-binding
- WHEN the request is made
- THEN the response is 403 with code execution_binding_scope.

### Requirement: Served contract matrix

r[kiln.web.http_contract.served_matrix] The lockstep client fixture MUST be exercised over HTTPS against the actual `kiln-web-service serve` process. Successful bodies MUST validate against the committed contract; authentication (`401`), authorization (`403`), not-found (`404`) and other-status responses such as a bound violation (`400`) MUST keep distinct classifications; and a run listing MUST follow `next_cursor` to its end or be reported incomplete when a follow-up is refused or lost.

#### Scenario: Served error variants stay distinct

r[kiln.web.http_contract.served_matrix.scenario.errors]
- GIVEN the real service process with operator, viewer and worker tokens
- WHEN the client sends a wrong token, a worker request, a viewer execution-binding request, an unknown run and an out-of-range limit
- THEN it MUST classify them as authentication, authorization, authorization, not found and unknown, each with the served error code

#### Scenario: Pagination is complete or explicitly incomplete

r[kiln.web.http_contract.served_matrix.scenario.pagination]
- GIVEN more runs than one page holds
- WHEN the client follows `next_cursor`
- THEN it MUST either reach a page without `next_cursor` or report the listing incomplete, never a shorter complete listing

### Requirement: Bounded network client

r[kiln.web.http_contract.client_bounds] The client MUST bound the received bytes of success and error bodies before parsing, independently of `Content-Length`, MUST enforce named connect, total and body-idle deadlines, and MUST NOT retry. Oversized, truncated, mislabeled, slow or stalled bodies and connection failures MUST be transport outcomes, never authentication, authorization or schema outcomes.

#### Scenario: Faulty bodies are transport failures

r[kiln.web.http_contract.client_bounds.scenario.transport]
- GIVEN a loopback TLS stub returning an oversized chunked body, a `Content-Length` that disagrees with the body, a slow body or a stalled body
- WHEN the client reads the response
- THEN it MUST stop at the byte limit or deadline, classify the response as transport and send no second request

#### Scenario: Malformed bodies are schema mismatches

r[kiln.web.http_contract.client_bounds.scenario.schema]
- GIVEN a `2xx` body with a missing required field, a wrong type, a forbidden extra field or truncated JSON within the byte limit
- WHEN the client validates it
- THEN it MUST classify the response as a schema mismatch

### Requirement: Measured client generation decision

r[kiln.web.http_contract.generation_evaluation] A decision to adopt or reject a generated client for the contract MUST cite retained measurements, taken against the committed document at a pinned generator revision, of generated-code volume, added dependencies and their licenses, and cold and warm build time, next to the handwritten client fixture's size.

#### Scenario: The decision cites retained measurements

r[kiln.web.http_contract.generation_evaluation.scenario.measured]
- GIVEN the committed contract and the pinned generator
- WHEN a client is generated in a scratch workspace and built cold and warm
- THEN the measurements MUST be retained as evidence and the documented decision MUST cite them

### Requirement: Contract drift policy

r[kiln.web.http_contract.drift] CI MUST compare the committed contract with a reviewed baseline using pinned Drift. An incompatible change MUST fail unless the contract version differs from the baseline's, an unhandled comparison MUST fail, and a trivial change MUST pass with every change printed.

#### Scenario: Each classification has its policy

r[kiln.web.http_contract.drift.scenario.policy]
- GIVEN fixture document pairs that Drift classifies as incompatible, unhandled, trivial and incompatible with a changed contract version
- WHEN the CI check runs
- THEN the first two MUST fail and the last two MUST pass
