# HTTP API contract pilot

This document describes the bounded read and reviewed runbook-dispatch HTTP
contract for Kiln's web service, version `kiln.http-api.v1`. The
machine-readable contract is the
canonical OpenAPI 3.0.3 document `contracts/http-api/openapi.json` (sorted keys,
two-space indentation, trailing newline). The archived lifecycle change is
`.cairn/archive/1970-01-01-pilot-http-api-contract/`.

## Owned family

- `GET /api/projects`
- `GET /api/runbooks`
- `POST /api/runbooks/{id}/dispatch`
- `GET /api/runbooks/dispatches`
- `GET /api/runbooks/dispatches/{run_id}/rollup`
- `POST /api/runbooks/{id}/fanout`
- `GET /api/runbooks/fanouts`
- `GET /api/runbooks/fanouts/{fanout_id}`
- `GET /api/runbooks/fanouts/{fanout_id}/rollup`
- `POST /api/runbooks/fanouts/{fanout_id}/pause`
- `POST /api/runbooks/fanouts/{fanout_id}/continue`
- `POST /api/runbooks/fanouts/{fanout_id}/cancel`
- `GET /api/runs`
- `GET /api/runs/{id}`
- `GET /api/runs/{id}/execution-binding`
- `GET /api/runs/{id}/artifacts`

`POST /api/runbooks/{id}/dispatch` and `POST /api/runbooks/{id}/fanout` are
idempotent on `request_id` (bound to runbook ID, principal ID, and request body;
mismatch returns `409 request_id_conflict`, exact replay returns the original
`201`/`202` status with `"replayed": true`) and require an explicit, non-expired
principal grant for `mutating` runbooks (`403 runbook_grant_required`).

`POST /api/runbooks/fanouts/{fanout_id}/{pause,continue,cancel}` are similarly
idempotent on `request_id` (bound to fan-out ID, control action, principal ID,
and request body), enforce live `authorize_runbook_principal`, verify mode and
definition parity on `continue` (`409 runbook_mode_changed` /
`409 runbook_definition_changed`), and stop or resume future child admission
without claiming to stop already-running children. Mutating fan-outs check live
`<owner>/<repo> <path>` fleet claim overlap (`409 fleet_claim_conflict` for
`refuse`, per-child `skipped` for `skip`, or deferred pending admission for
`serialize`). Per-child outcomes preserve `pending`, `active`, `success`,
`failure`, `unknown`, and `skipped` explicitly without cancelling siblings or
retrying blindly.

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

The read-only roll-up endpoints (`GET /api/runbooks/dispatches`,
`GET /api/runbooks/dispatches/{run_id}/rollup`, and
`GET /api/runbooks/fanouts/{fanout_id}/rollup`) are bounded (`limit` `1..=100`,
cursor-paginated), admit `viewer` and `operator` (refusing `worker` with
`403 worker_scope`), preserve `Outcome::Unknown` and explicit
`present`/`missing`/`unrun`/`unavailable` receipt states, validate `https://`
PR/stack links without userinfo, and always emit `grants_authority: false` and
`non_claims`.

## Client bounds

The lockstep test client in `crates/kiln-web-service/tests/http_contract/served_client.rs`
uses the fixture CA to talk to the real HTTPS service and a loopback TLS stub.
For every status, it incrementally caps received body bytes at `CLIENT_BODY_LIMIT`
(64 KiB) before parsing, regardless of `Content-Length`. Named deadlines use
3 s connect, 10 s total and 3 s body-idle for the real service (including TLS
and persistence), and 400 ms connect, 900 ms total and 250 ms body-idle for
the controlled stub faults. Each request makes exactly one attempt (no retries).
Mismatched framing, an oversized body,
timeouts and connection failures classify as `transport`; malformed or
schema-invalid 2xx bodies classify as `schema_mismatch`. Other classes remain
`authentication` (401), `authorization` (403), `not_found` (404), `unknown`
(other statuses), and `ok` (validated 2xx). These are test-fixture bounds, not
a production client or a hostile-network qualification.

## Lockstep compatibility matrix

Decision: lockstep for this family. Tested window: current server and current
client only; retained schemas do not promise every prior API version. Tests run
against both real `Engine::api` responses and the actual `serve` process over
loopback TLS:

- all five GET success bodies validate against the committed document with an
  independent schema-subset validator that fails closed on unsupported keywords
  or unresolved references; the served shell also exercises real 401, 403,
  404 and 400 responses with their distinct codes and classes;
- a listing with more than one page follows `next_cursor` to the end; refused
  cursors and lost follow-ups remain incomplete, never a shorter complete list;
- a loopback TLS stub replays a real served body with missing required, wrong
  type, forbidden extra field, or truncated JSON mutations (`schema_mismatch`),
  and mismatched `Content-Length`, oversized chunked success and error bodies,
  slow drip and stalled-body faults (`transport`), counting requests to prove
  there is no retry;
- the committed error goldens under `crates/kiln-web-service/tests/http-contract/`
  must equal the real in-process responses.

## Drift and Progenitor evaluation

The isolated `tools/http-contract-drift` binary pins Drift
`dad4f398c10ca53298d2fec0586d834c8e85daf1` (Apache-2.0). CI's
`checks.x86_64-linux.http-contract-drift` compares the committed document with
`contracts/http-api/baseline/openapi.json` and checks four minimal policy pairs.
It prints every Drift record in deterministic order. A trivial change (or no
change) passes; backward-, forward- and both-direction incompatible changes
fail unless `info.version` differs; unhandled changes always fail. Exit 0 is a
pass, 1 a policy rejection and 2 a tool/document error. The fixture check
asserts the exact status, class and decision, rather than treating an error as
the expected rejection.

A changed contract version opens a **pending transition**: incompatibilities
against the old baseline pass while the new version is unaccepted, even for
later changes; unhandled changes still fail. To accept a reviewed transition,
replace the baseline with the accepted canonical OpenAPI document in that same
reviewed change. That closes the pending transition; future incompatible edits
to this version fail again. For an unhandled comparison, inspect the actual
change and Drift's limitation before replacing the baseline in the same
reviewed change. For an otherwise compatible change, review the printed record
and update the baseline when accepting the new contract, so subsequent reviews
compare against the last accepted document. Never update the baseline merely
to silence an unexpected policy failure.

Progenitor `b317a86578098ecabbeb04a0f57dc693032b81cc` (MPL-2.0) was
evaluated using its documented `build.rs` path in a scratch crate outside Kiln.
Reproduce the measurement inside `nix develop` with
`python3 scripts/http-contract-generation-measure.py --jobs 2 --output receipt.json`;
the script creates and removes its scratch crate under
`$HOME/.cache/kiln/http-contract-generation/` (`--scratch` overrides it) and
writes the JSON receipt without changing the root Cargo workspace.
The unchanged committed contract lacks operation IDs and generation fails with
`UnexpectedFormat("path /api/projects is missing operation ID")`: it emits no
client, so successful build time or generated volume cannot be claimed for that
document (failed cold attempt: 43.89 s).

A scratch-only copy adding `operationId` to each of the five GET operations
(the five precise names/paths are in the retained evidence) **does** generate
and build: **470 Rust lines, 16,940 bytes**, 57.41 s cold from a fresh target
and 0.17 s for an unchanged warm rebuild, at two Cargo jobs. The scratch lock
resolves eight direct and 192 transitive packages (74 package name/version pairs
absent from Kiln's root lock), including three MPL-2.0 Progenitor crates;
their individual licenses are inventoried in
[`generation-measurement.json`](../.cairn/archive/1970-01-01-qualify-http-contract-pilot/evidence/generation-measurement.json).
The handwritten validator/classifier in
`crates/kiln-web-service/tests/http_contract.rs:350-494` is 145 lines; the
generated output does not replace its runtime bounds and schema validation.
Decision: **no-go for generated-client adoption** in this pilot: it cannot
generate from the committed document without maintaining five extra operation
IDs, and the adapted client adds 470 generated lines and an isolated dependency
cohort while retaining client-boundary validation work. Single-host build times
are observations, not performance guarantees. Drift remains a CI-only tool
with its separate lock and [license inventory](../LICENSES.md).

## Non-claims

This contract does not replace the server, change canonical CI identities, or
make OpenAPI a provider-authorization mechanism.

