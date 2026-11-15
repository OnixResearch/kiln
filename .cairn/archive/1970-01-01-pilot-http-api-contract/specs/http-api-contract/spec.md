# HTTP API Contract Delta

## ADDED Requirements

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
