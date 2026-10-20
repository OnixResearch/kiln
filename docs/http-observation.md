# HTTP observation shell

`kiln-adapter-http-observe` serves the accepted `kiln-observe-core` projection
over bounded HTTPS. It is a read-only transport shell: it adds no view
vocabulary, no projection logic, no control intake, and no canonical state.

## Profile

`config/http-observation-profile.ncl` declares one registered HTTPS origin, the
TLS certificate chain and private-key paths, the observation-auth policy, the
JSON and SSE content contracts, the live-stream bounds, and the request bounds.
It also embeds the accepted observation profile used for projection.

Admission rejects a non-HTTPS origin, uppercase or malformed hosts, port zero,
relative TLS paths, unsupported TLS minimum versions, unsupported auth kinds,
malformed principals, zero bounds, defaults above the projection bounds, and
missing required non-claims.

## Routes

| Method and path | View |
|---|---|
| `GET /v1/views/state-summary` | state summary |
| `GET /v1/views/runs?after=<run>&limit=<n>` | paged run list |
| `GET /v1/views/runs/<run>` | run detail |
| `GET /v1/views/attempts/<attempt>` | attempt detail |
| `GET /v1/views/events?limit=<n>` | event-tail window |
| `GET /v1/views/logs/<stream>?cursor=<n>&max_bytes=<n>` | log range |
| `GET /v1/live?after=<revision>&limit=<n>` | bounded SSE event stream |

Only `GET` is admitted. Unknown paths return a bounded `unknown_view` rejection.
Unknown run, attempt, and log-stream targets return a bounded `unknown_target`
rejection before projection. The `Host` header must match the registered origin.

## Authentication

Every request is admitted against the profile auth policy before projection.
The reference verifier is `ed25519_request_v1`:

```text
Authorization: Kiln-Observe <base64url(payload-json)>.<base64url(signature)>
```

The Ed25519 signature covers the exact compact payload bytes:

```json
{
  "schema": "kiln.observe.http.auth.v1",
  "method": "GET",
  "path": "/v1/views/runs?limit=2",
  "body_blake3": "<BLAKE3 of the empty body>",
  "nonce": "<caller nonce>",
  "issued_at": 1700000000,
  "state_revision": 3
}
```

The server checks the schema, method, full target including query, body digest,
state revision, issue-time skew, nonce bound, and signature. Each admitted nonce
is retained in a bounded replay cache for the declared replay window. A failed
check returns a bounded `401 unauthorized` response without running projection.

The Ed25519 profile principal is either 64 lowercase hexadecimal characters or
a `did:key` Ed25519 multicodec reference.

The profile also admits the optional `ucan_holder_invocation_v1` backend:

```text
Authorization: UCAN <base64url(compact-token)>.<base64url(signature)>.<base64url(nonce)>
```

The profile declares the root issuer DID, authorized holder keys, observation
resource and ability, operation profile `kiln.observe.http.v1`, bounded proof
tokens, revoked proof references, and token/proof/nonce/field bounds. The server
verifies the compact token, proof chain, revocation status, capability,
operation profile, request binding over the method, full target, body digest and
state revision, token time window, holder signature, and one bounded replay
commit. Every token must be issued by the profile root or delegated from it
through the configured anchored proof set; holder keys cannot self-issue
observation authority. Caveats are denied in this backend. The UCAN cohort is vendored at
`d46816f19606b518307ba21ab33e8b93b6ebf646`; the root `ucan` crate is
`AGPL-3.0-or-later`, and `ucan-core` and `verified-logic` are `MPL-2.0` at that
revision. Basalt policy authority remains a deferred follow-up.

## TLS

The server loads the configured PEM certificate chain and private key, requires
TLS 1.2 or newer (or TLS 1.3 only), and does not request client certificates.
Read and write timeouts bound each connection. A failed handshake closes that
connection and does not stop the listener.

`fixtures/http-observation/tls/` contains a test-only CA, server chain, and
private key. They are committed test material, not production credentials.

## JSON views

Admitted views are encoded as compact canonical JSON of the accepted
`ObservationView` value. Responses carry:

- `Content-Type: application/json`;
- `X-Kiln-View-Identity: <kiln.observe.view.v1 BLAKE3 identity>`;
- `X-Kiln-Observe-Profile: <profile id>`;
- `Cache-Control: no-store`;
- `Connection: close`.

The JSON body carries the same view identity as the accepted projection. The
HTTP shell does not recompute or replace it.

## Live SSE

`/v1/live` is a finite, bounded SSE response. The shell owns a drop-oldest event
buffer capped by the profile. Frames use `event: observe` with a projected
event-tail view and `id: <next revision>`, or `event: lag` with the cumulative
dropped count and retained revision range. The stream closes at the declared
event-count or duration bound; clients reconnect with `after=<next revision>`.

A dropped or delayed fact is a shell observation. It never changes canonical
state and is reported only through the `lag` frame and the bounded reconnect
contract. Live delivery is not a completeness claim.

## Rejections

Rejections carry `application/json` and a bounded
`{"schema":"kiln.observe.http.error.v1","code":...,"message":...}` body:

| Status | Meaning |
|---|---|
| `400` | malformed request, origin mismatch, invalid query, target, or bounds |
| `401` | missing, stale, replayed, or invalid authorization |
| `404` | unknown view kind, run, attempt, or log stream |
| `405` | non-GET method |
| `408` | request read timeout |
| `413` | request body or total request exceeds its bound |
| `431` | header bytes or field count exceeds its bound |
| `500` | projection or profile state could not be served |

## Evaluation outcome

`host-control-core` was evaluated and is not adopted for this change. It is a
pure host-topology and host-event DTO crate with no HTTP request, response, or
SSE frame contract; mapping CI observation into its topology commands would
invent a lossy second vocabulary. No host-control source is imported, so no pin
or third-party notice is added.

UCAN is now imported as the exact vendored cohort
`d46816f19606b518307ba21ab33e8b93b6ebf646` and used by the optional
`ucan_holder_invocation_v1` auth backend. Basalt policy authority is still not
imported; basalt adds bounded-http, replay-ledger, revocation, and secretspec
beyond the UCAN holder/capability boundary. A future Basalt import must record
the exact source revision, license map, third-party notices, and runtime cohort
before binding it behind the same auth port.

## Non-claims

Passing HTTP observation checks do not prove transport acceptance, live
completeness, control execution, provider correctness, CI success, or release
readiness. They prove only the checked profile, request, projection, bounds,
authorization decision, and stream observation.
