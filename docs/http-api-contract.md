# HTTP API contract pilot

This document describes the bounded GET-only HTTP contract for Kiln's web
service, version `kiln.http-api.v1`. The machine-readable contract is the
canonical OpenAPI 3.0.3 document `contracts/http-api/openapi.json` (sorted keys,
two-space indentation, trailing newline). The archived lifecycle change is
`.cairn/archive/1970-01-01-pilot-http-api-contract/`.

## Owned family

GET only:

- `GET /api/projects`
- `GET /api/runs`
- `GET /api/runs/{id}`
- `GET /api/runs/{id}/execution-binding`
- `GET /api/runs/{id}/artifacts`

No mutating endpoint is part of this pilot.

## Authentication and admission

Roles are `viewer`, `operator` and `worker`. The bounded service rejects:

- worker access to non-worker surfaces with `403 worker_scope`;
- non-worker access to worker-only upload/cache operations with
  `403 worker_only`;
- viewer access to `execution-binding` with `403 execution_binding_scope`.

Authentication/admission failures are distinct from transport failure and from
schema mismatch. A transport failure returns no parsed contract body; a schema
mismatch returns a 2xx body that fails independent validation.

## Status and error bodies

Errors use:

```json
{"code":"...","message":"..."}
```

Observed codes include `request_bound`, `query_invalid`, `cursor_invalid`,
`order_invalid`, `page_bound`, `outcome_invalid`, `run_missing`,
`execution_binding_scope`, `worker_scope`, `worker_only`, `route_missing`.

## Nullable and optional fields

The contract marks nullable fields with OpenAPI 3.0 `nullable: true`
(`Project.scope`, `RunPage.next_cursor`, `ExecutionBinding.execution`).
Only `ArtifactList`, `Error`, `ProjectList` and `RunPage` reject additional
properties. `RunView`, `Project`, `ExecutionBinding` and `LogBounds` admit them,
so additive server fields do not break the lockstep client.

## Size limits and pagination

- request body cap: `max_request_bytes`;
- run list `limit` must be 1..=100, default 50 for non-workers and
  `max_runs` for workers;
- cursor validation binds version, query fingerprint, order, ordinal and run
  identity;
- `next_cursor` is absent when no further page exists;
- `bounded: true` is always present.

## Unknown outcomes and lost observation

`Outcome::Unknown` and lost observation remain explicit values. HTTP 2xx or
successful deserialization is not evidence of execution, completed cleanup or
accepted CI.

## Client bounds

The lockstep client fixture in `crates/kiln-web-service/tests/http_contract.rs`
bounds received bytes before parsing (`CLIENT_BODY_LIMIT`), performs no retries
and classifies a response as `authentication` (401), `authorization` (403),
`not_found` (404), `transport` (truncated or oversized body), `schema_mismatch`
(a 2xx body that is malformed or fails the independent validator), `unknown`
(any other status) or `ok`. Request deadlines belong to a networked client and
are not exercised by this in-process fixture.

## Lockstep compatibility matrix

Decision: lockstep for this family. Tested window: current server and current
client only; retained schemas do not promise every prior API version. Tests run
against real `Engine::api` responses, not serialized Rust types:

- every operation's success body, and the 400, 403 and 404 error bodies,
  validate against the committed document with an independent schema-subset
  validator that fails closed on unsupported keywords or unresolved references;
- mutated copies of a real body (missing required field, wrong type, extra
  field where `additionalProperties` is false, truncated JSON) classify as
  `schema_mismatch`, while oversized and truncated transfers classify as
  `transport`;
- the committed error goldens under `crates/kiln-web-service/tests/http-contract/`
  must equal the real responses.
## Drift and Progenitor evaluation

Evaluated in a scratch directory outside the workspace at Drift
`dad4f398c10ca53298d2fec0586d834c8e85daf1` (Apache-2.0) and Progenitor
`b317a86578098ecabbeb04a0f57dc693032b81cc` (MPL-2.0). No dependency was added
to the Kiln workspace; transitive licenses must be recorded before any adoption.

- Drift classification policy: incompatible changes require rejection or an
  intentional version transition; unhandled comparisons fail closed or require
  review; trivial changes are recorded and allowed only after review.
- Decision: no-go for adoption in this pilot. Progenitor's generated client
  would add dependencies and generated code without removing the need for
  runtime input validation, and Drift is only useful once a second contract
  version exists. Revisit when client churn or a versioned transition justifies
  it. Build time and generated-code volume were not retained as receipts.

## Non-claims

This contract does not replace the server, change canonical CI identities, or
make OpenAPI a provider-authorization mechanism.

