## Context

`kiln-adapter-http-observe` serves accepted observation views over HTTPS. Its
profile currently admits one auth variant, `ed25519_request_v1`, which verifies
a pinned Ed25519 signature over the request target, body digest, nonce, time,
and state revision. The observation design records UCAN holder invocation as the
planned capability backend behind the same auth boundary.

The UCAN source is a separate AGPL-licensed repository with an MPL-2.0
`verified-logic` snapshot. Kiln standalone builds must not depend on a sibling
checkout or operator credentials, so the cohort is vendored at an exact
revision with retained license and notice material.

## Decisions

### Decision: Vendor the exact UCAN cohort

**Choice:** Copy the UCAN repository snapshot at revision
`d46816f19606b518307ba21ab33e8b93b6ebf646` into `vendor/ucan`, including
`crates/ucan-core`, `crates/ucan-nostr`, `src`, `tests`, `docs`, the root
`Cargo.toml`, and `vendor/verified-logic`. Record the source revision, license,
and third-party notices. Use a path dependency from the HTTP adapter.

**Rationale:** The source is private and the workspace must build without
sibling checkouts or credentials. Vendoring preserves the exact reviewed bytes
and keeps the AGPL/MPL boundary visible. A Git dependency would add a network
and credential dependency to standalone builds.

### Decision: Add a second auth variant

**Choice:** Extend the HTTP auth profile with `ucan_holder_invocation_v1` while
keeping `ed25519_request_v1`. The profile `kind` selects the verifier.

**Rationale:** Existing accepted profiles and deployments remain valid. The
change is additive and the observation transport contract does not change.

### Decision: Require holder-signed invocation

**Choice:** The client presents a compact UCAN token, an Ed25519 holder
signature, and a 32-byte nonce. The server verifies the root-anchored token and
proof chain, revocation, capability, operation profile, request binding, time
window, and holder signature before admitting the request.

**Rationale:** A bearer token alone does not prove possession of the audience
key. Holder-signed invocation binds the exact request and nonce, and the UCAN
API performs the state-consuming replay commit last.

### Decision: Anchor every token to the profile root

**Choice:** Profile admission rejects configured proof tokens that do not form
a chain rooted at the profile issuer. At request time, the header token must be
issued by the profile root or by the audience of an anchored configured proof
token. Holder keys authenticate the holder signature and may participate in
anchored delegations, but cannot self-issue observation authority.

**Rationale:** The vendored verifier validates signatures and proof-chain links
but treats trust roots as caller-owned. Without an explicit Kiln-side anchor, a
holder key could self-issue a capability and satisfy signature and capability
checks without root delegation.

### Decision: Use a fixed observation operation profile

**Choice:** The operation-profile identity is the BLAKE3 digest of
`kiln.observe.http.v1`. The request-binding digest is BLAKE3 over canonical
JSON containing the method, full target, body digest, and state revision. The
profile capability resource and ability are explicit values such as
`kiln:observe` and `observe`.

**Rationale:** UCAN does not interpret downstream action semantics. Kiln owns
the exact observation request binding and capability meaning.

### Decision: Fail closed on caveats and revocation

**Choice:** The backend admits no caveats in this change and rejects unknown
caveats. Proof references listed as revoked in the profile are denied. Proof
tokens, proof depth, compact-token bytes, nonce bytes, and field lengths are
bounded.

**Rationale:** A bounded, explicit first backend is safer than silently
ignoring unsupported caveats or revocation. Caveat policy can be added later
behind the same profile.

### Decision: Reuse the bounded replay boundary

**Choice:** Implement `HolderInvocationReplayAdmission` over the existing
bounded replay state, keyed by audience, nonce, and request digest. The UCAN
verifier calls it exactly once after all other checks.

**Rationale:** Replay is the only state-consuming boundary in holder
verification. Keeping it bounded and final preserves the existing shell's
failure semantics.

## Risks / Trade-offs

- AGPL/MPL linkage widens the combined-artifact license boundary. The license
  map and notices must be updated before archive.
- UCAN source is large and verified-logic adds a substantial dependency cohort.
  The exact vendored snapshot is the reviewed boundary; no sibling path is used.
- Caveats are denied rather than partially interpreted. A future change must
  add explicit caveat policy before claiming broader capability semantics.
- Clock, proof-chain, and replay truth remain caller-owned; the shell supplies
  explicit time and bounded state, not global freshness.

## Claim Boundary

Passing checks prove only the checked profile, compact token, proof chain,
revocation list, holder signature, operation profile, request binding, time
window, replay admission, and observation projection. They do not prove issuer
intent, key custody, global freshness, source trust, provider correctness, CI
success, or release readiness.
