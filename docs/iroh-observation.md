# Iroh observation shell

`kiln-adapter-iroh-observe` serves the accepted `kiln-observe-core` projection
to enrolled iroh peers over one `kiln.observe.v1` ALPN protocol. It is a
read-only transport shell: it adds no view vocabulary, projection logic, control
intake, or canonical state.

## Cohort and profile

The consumed cohort is pinned to `iroh` `0.95.1`. `iroh-blobs` and `bao-tree`
are recorded as deferred; the shell uses a bounded one-stream request/response
frame and a Kiln-owned BLAKE3 payload identity instead. A future import must
record the exact verified-streaming cohort and review before replacing that
path.

`config/iroh-observation-profile.ncl` declares:

- the exact cohort facts and the `kiln.observe.v1` ALPN;
- the endpoint bind address, relay URLs, connection, frame, payload, and
  timeout bounds;
- the allowed and revoked peer endpoint identities and the peer-count bound;
- the accepted observation profile used for projection;
- every required non-claim.

Admission rejects cohort drift, an unsupported ALPN, relative or non-positive
bounds, non-HTTPS relay URLs, empty or oversized peer sets, and missing required
non-claims before an endpoint binds.

## Peer identity and trust

Iroh authenticates each connection with the peer's Ed25519 endpoint key. The
profile allowlist is the trust policy. Revocation takes precedence over
allowance. Unknown and revoked peers are denied before a request is read.

Each endpoint identity has a deterministic `did:key` Ed25519 form via
`did_key(endpoint)`. This aligns the transport principal with the capability
principal model without importing UCAN. The shell records the endpoint identity;
it does not claim peer intent or capability authority.

Relay URLs are explicit and optional. Empty relay URLs disable relays. When a
relay is configured, relay fallback is a visible transport fact and grants no
authority; the observation view contains no relay-dependent canonical fact.

## Wire contract

One bidirectional stream carries one length-prefixed request and one
length-prefixed response, both bounded by the profile frame bound.

Request schema: `kiln.observe.iroh.request.v1`, containing the accepted
`ObservationRequest`.

Response schema: `kiln.observe.iroh.response.v1`, containing either:

- `ok`: the accepted `ObservationView` and an optional `PayloadIdentity`;
- `err`: a bounded stable code and message.

The view carries its accepted `kiln.observe.view.v1` identity. The shell does
not recompute or replace it.

## Verified payloads

Log ranges with returned bytes carry a `log_bytes` payload identity: the BLAKE3
digest of the exact returned bytes. Event-tail views carry an `event_records`
payload identity: the BLAKE3 digest of the canonical JSON encoding of the
returned record projections.

The recipient recomputes the identity from the served view before claiming
delivery. A missing, changed, unknown, or mismatched identity is rejected and is
not reported as delivered. The server also recomputes the identity before
encoding the response.

This proves payload identity over the admitted bytes. It does not prove source
truth, completeness, or that the peer honestly retained the payload.

## HTTP equivalence

`admit_equivalence` compares the logical `ObservationView` values served by the
HTTP and iroh legs over shared fixtures. The maintained test projects one shared
snapshot/request, serves the iroh leg through a local endpoint pair, and compares
the transported view with the HTTP adapter's canonical encoding. Divergence
fails conformance. Transport framing and relay facts are outside the logical
view comparison.

## Bounds and failure

- endpoint connections, frame bytes, payload bytes, request timeout, and write
  timeout are bounded by the profile;
- an oversized, malformed, trailing, or schema-mismatched frame is rejected;
- transport failure, timeout, and peer denial are bounded errors and never
  return a `view`;
- denied peers receive a connection denial before request admission;
- no transport error changes canonical state, run outcomes, gates, or release
  readiness.

## Non-claims

Passing iroh observation checks do not prove transport acceptance, live
completeness, verified delivery of unverified payloads, control execution,
provider correctness, CI success, or release readiness. They prove only the
checked profile, protocol, peer, payload, bounds, and view identity.
