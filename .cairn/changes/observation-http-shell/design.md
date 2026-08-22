# Design: HTTP observation shell

## Context

The accepted observation core projects views from committed state. This change serves those views to standard clients over HTTPS. It is the first concrete transport for the projection and the foundation of any Buildbot-like operator surface.

The shell must follow the repo's transport discipline: pure decision core, thin imperative shell, one registered origin, no ambient authority, and loss that never touches canonical state.

## Goal and Success Contract

The goal is a bounded HTTP shell that serves the accepted projection with canonical JSON views, a live SSE stream, and strict admission.

Completion requires a typed profile, a serving shell, live delivery and loss handling, auth, TLS, full positive and negative conformance, and documented non-claims.

False completion includes a view served without its `kiln.observe.view.v1` identity, an unbounded live buffer, an unauthorized view, a request-supplied destination, or a leak of transport failure into canonical state.

## Decisions

### Decision: Serve the accepted projection unchanged

`kiln-adapter-http-observe` consumes the accepted observation core. It adds no view vocabulary and no projection logic. The shell maps HTTP requests to admitted view requests and encodes admitted views canonically.

Rejected alternative: a second projection implementation in the shell. It would fork the projection contract and break rebuild parity.

### Decision: One registered origin

The profile names exactly one registered HTTPS origin. Requests cannot supply destinations. Redirect, DNS-class, loopback, and private-address policy follows the accepted webhook adapter discipline.

### Decision: Canonical JSON with identity

Views encode with the same canonical encoding as the accepted projection and carry their `kiln.observe.view.v1` identity. Encoding must be deterministic and rebuildable.

### Decision: Live stream from committed events

The SSE stream advances only from committed journal events. The shell owns the bounded buffer and drop policy. A dropped or delayed fact is a stream observation; it never changes canonical state and is reported under the declared policy.

Rejected alternative: deriving live facts from a mutable projection cache. A cache may exist as a shell-owned observation, but it must never become a source canonical transitions read.

### Decision: Auth before projection

Every request is admitted against the profile auth policy before any projection runs. Auth failure returns a bounded rejection. Consumers retain nonce, clock, replay, and transport authority.

Planned reference implementation: observation authority through a versioned observation-authn port using the basalt boundary contract with ucan enforcement. Nickel policy declares who may observe which project and tenant scope; UCAN capabilities grant bounded observation rights; principals are `did:key` Ed25519. The spec keeps the requirement policy-neutral; the technology choice is a design fact subject to the cross-stack evaluation below.

### Decision: Rejections are bounded and specific

Unknown view kinds, unknown targets, oversized requests, TLS failures, and timeouts each return a bounded, specific result. None claim a projection or an effect.

### Decision: No write authority

The HTTP shell performs no command intake. Controls are a later change through the existing command and outbox path.

## Cross-stack evaluation and reuse

Sibling reuse follows the OnixResearch owner grant: public license conflicts do not block reuse of source the stack exclusively controls. Third-party notices and vendored material (including n0 crates, celld, and the vendored verified-logic snapshot) remain documented and preserved.

- `host-control-core` provides snapshot-first streams and bounded observations and is already consumed by tile through `tile-host-control-adapter`. It is adopted as the wire-contract framing substrate for this leg; CI content stays in Kiln-owned payloads under the host-control product-neutral boundary. Adoption scope is confirmed by an evaluation milestone that gates the wire-format freeze, never the pure projection core.
- Observation authority uses the basalt boundary with ucan enforcement behind the observation-authn port as described in the auth decision above.
- `valence` (through the existing adapter) may record each served view as `recorded_only` evidence bound to its `kiln.observe.view.v1` identity.

Recorded pins follow the existing accepted-spec discipline.

## Configuration

Typed Nickel profiles declare the registered origin, TLS policy, auth admission, content and SSE contracts, named bounds, and non-claims. Positive and negative profile fixtures validate the contract.

## Validation

Positive fixtures cover admitted serving, identity-bearing views, live delivery, and authorized access. Negative fixtures cover drift, unknown kinds, unknown targets, unauthorized requests, malformed requests, TLS failure, timeouts, stream loss, missing negative cases, and authority promotion.

## Claim Boundary

Passing checks establish bounded HTTP serving of the accepted projection. They do not prove transport acceptance, live completeness, control execution, provider correctness, CI success, or release readiness.
