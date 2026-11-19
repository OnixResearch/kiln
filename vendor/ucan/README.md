# ucan

## Architecture

- `crates/ucan-core`: `#![no_std]` functional core. Keep parsing, structural validation, and other deterministic UCAN logic here.
- `ucan` (repo root): `std` shell. Keep file I/O, process integration, networking, clocks, crypto, and other side effects here.
- `vendor/verified-logic`: vendored snapshot of the verified library used by the `std` shell for UCAN ability subsumption, attenuation checks, exact finite capability-set authorization, proof-chain issuer/audience linkage, proof-depth budget admission, finite revocation-set membership, concrete invocation admission, deterministic receipt executor/timestamp checks, finite typed caveat helpers, finite proof traversal transcript checks, Web-of-Trust principal digest derivation/admission, WOT evidence/policy helpers, WOT UCAN authorization, token-time validation, Ed25519 signing/verification, and `did:key` encoding/decoding.

The boundary is FCIS-first: the shell gathers bytes and environment state, then hands plain strings and values into `ucan-core`. When a primitive needs verified semantics, the shell bridges validated core values into `verified-logic` instead of reimplementing the logic locally.

The shell crate now carries `serde`, `serde_json`, `base64`, `blake3`, and `snafu` as the correctness-focused foundation for compact-token parsing, strict claim decoding, proof references, and typed error reporting. Property-based integration coverage uses Hegel via the `hegeltest` crate.
The development shell applies `tests/hegel-constraints.txt` to fresh UV bootstrap.
Hegel 0.2.3 is tested here with Hypothesis 6.148.5; 6.167.1 causes a server error.
For an existing incompatible test environment, run
`uv pip install --python .hegel/venv/bin/python -c tests/hegel-constraints.txt hegel-core==0.2.3`
inside the development shell. This changes only test infrastructure, not UCAN
verification or runtime dependencies.

## End-to-end issue/parse/verify

The primary public workflow is token-oriented:

```rust
use ucan::{
    AudienceDid, CapabilitySet, ED25519_SECRET_KEY_BYTES, Ed25519InMemorySigner,
    KeyResolutionContext, ProofCollection, ProofReferences, TokenSigner,
    TokenTimeBounds, VerificationContext, VerificationTime,
    issue_token_with_signer, verify_compact_token,
};

let issuer = Ed25519InMemorySigner::from_seed_bytes([7; ED25519_SECRET_KEY_BYTES]);
let audience_key = Ed25519InMemorySigner::from_seed_bytes([9; ED25519_SECRET_KEY_BYTES]);
let audience = AudienceDid::from(audience_key.issuer()?);
let capabilities = CapabilitySet::single("did:key:z6Mkresource", "store/add")?;
let proof_references = ProofReferences::empty();
let time_bounds = TokenTimeBounds::new(1_000, 2_000)?;
let token = issue_token_with_signer(
    &issuer,
    &audience,
    &capabilities,
    &proof_references,
    time_bounds,
)?;
let context = VerificationContext::new(
    VerificationTime::from_unix_seconds(1_500),
    KeyResolutionContext::new(vec![issuer.verification_key()?]),
    ProofCollection::empty(),
);
let verified = verify_compact_token(&token, &context)?;
assert_eq!(verified.capabilities().as_slice().len(), 1);
assert!(verified.authorize("did:key:z6Mkresource", "store/add").is_allowed());
assert!(!verified.authorize("did:key:z6Mkresource", "admin/delete").is_allowed());
assert!(!verified.authorize("did:key:z6Mkresource", "store/*").is_allowed());
# Ok::<(), Box<dyn std::error::Error>>(())
```

For integration code with many inputs, prefer the named builders and boundary-only
time conversion helpers over positional constructors:

```rust
# use std::time::{Duration, UNIX_EPOCH};
# use ucan::{CapabilitySet, ED25519_SECRET_KEY_BYTES, Ed25519SigningKey, IssueRequest, ProofReferences, TokenTimeBounds};
# let signing_key = Ed25519SigningKey::from_bytes([7; ED25519_SECRET_KEY_BYTES]);
# let audience = ucan::AudienceDid::from(signing_key.issuer()?);
let request = IssueRequest::builder()
    .signing_key(signing_key)
    .audience(audience)
    .capabilities(CapabilitySet::single("did:key:z6Mkresource", "store/add")?)
    .proofs(ProofReferences::empty())
    .time_bounds(TokenTimeBounds::from_system_time_and_duration(
        UNIX_EPOCH + Duration::from_secs(1_000),
        Duration::from_secs(1_000),
    )?)
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Run the same happy path as a normal example:

```bash
cargo run --example issue_and_verify
cargo run --example http_bearer
cargo run --example failure_handling
```

After `verify_compact_token` succeeds, use `VerifiedToken::authorize`,
`VerifiedToken::authorize_request`, or `VerifiedToken::authorize_with_policies`
to evaluate a concrete operation against the effective delegation only. Concrete
request admission rejects wildcard request abilities through the same
`VerifiedCapability` / `verified-logic` ability bridge used for delegation
containment rather than a parallel shell-local ability parser. Authorization
decisions are structured: allowed decisions expose the matched effective
capability, while denied decisions separate malformed requests (for example a
requested ability containing a `*` segment), missing caveat policies, rejected
caveats, and requests with no matching effective capability. Caveats are opaque
domain/type/key/payload records preserved in `ucan-core` and evaluated in the
shell through caller-supplied `CaveatPolicySet`; unknown caveats fail closed.
For callers that explicitly adopt the vendored typed WOT caveat vocabulary,
`typed_caveat_is_satisfied` and `typed_caveat_set_is_satisfied` expose finite
time-bound and rate/count-window predicates over caller-supplied context without
installing default caveat policies or changing opaque shell caveat behavior.

`verify_invocation` is an audience-only compatibility helper. It verifies the compact token and authorizes resource and ability against the token audience. It does not prove sender or holder possession, and `InvocationAllowed` does not expose an invoker claim.

Use `sign_holder_invocation` and `verify_holder_signed_invocation` when a live sender must prove possession of the verified audience key. The borrowed verifier binds the complete bounded token BLAKE3 reference, verified audience DID, resource, ability, fixed operation-profile identity, exact request-binding digest, and fixed nonce in the `ucan/holder-invocation/v1` canonical format. It confirms that the resolved public key encodes the verified audience DID. It then verifies the audience signature before non-consuming profile and caveat observations. It calls `HolderInvocationReplayAdmission` once as the final state-consuming commit point and returns a private-construction, non-serde, non-clone `HolderInvocationAuthorized` value.

For an ongoing operation, `recheck_holder_authorization` borrows the original live
holder authorization and rechecks the same token, proofs, revocation, profile,
and caveats without a replay port. Its borrowed observation is not another
invocation or continuous authority. See [the recheck boundary](docs/holder-authorization-recheck.md).

`OperationBinding` lets Basalt map its closed action enum to a stable `OperationProfileId` and exact `RequestBindingDigest`. UCAN does not interpret downstream action or request semantics. Profile and caveat checks must not spend quota or mutate admission state. Consumers must apply consumptive rate limits and execution effects in a separate downstream atomic boundary after UCAN success.

```bash
cargo run --example invocation
```

## Proof-chain delegation

Proof material is explicit. A child token carries BLAKE3 proof references in `prf`; issuance accepts `ProofReferences`. Verification accepts `ProofCollection` or a caller-owned synchronous `ProofStore`. Verification checks each parent signature, proof-depth budget admission through `proof_depth_within_budget`, issuer/audience linkage through the `proof_link_is_valid` verified-chain bridge, temporal narrowing, capability attenuation, and self-delegation rejection before returning the effective delegation constraints. Advanced callers that already have resolved public chain fields can use `VerifiedChainEntry` plus `structural_chain_is_valid` to run the vendored structural chain composition checker over issuer/audience linkage, caller-provided signature-valid flags, exact attenuation narrowing, and temporal validity. If they also have aligned proof-reference bytes and finite revoked-reference bytes, `structural_chain_not_revoked_is_valid` runs the vendored composed non-revoked-chain checker over the same structural data plus reference-count alignment and finite revocation-set non-membership. For caller-owned finite capability sets, `capability_set_authorizes` runs the vendored capability-set authorizer with resource byte-prefix refinement and exact ability equality; it is not wildcard subsumption and does not replace shell token/effective-delegation authorization. For resolved concrete invocations, `VerifiedInvocationChainRequest` plus `invocation_chain_is_authorized` additionally checks explicit authorization-reference binding to the chain leaf, leaf capability index admission, invoker/audience equality, resource refinement, and ability equality through the vendored invocation-chain checker. For shell-produced finite traversal transcripts, `VerifiedProofTraversalTranscript` plus `proof_traversal_transcript_is_valid` checks resolved entries, visited-reference alignment, duplicate/cycle rejection, depth/proof-count/input-byte budgets, issuer/audience linkage, attenuation narrowing, ordered caveat-id preservation, target authorization, explicit time validity, and supplied revocation-denial facts. For finite Web-of-Trust authorization data, `web_of_trust_principal_digest` derives fixed-width public principal bytes and `web_of_trust_principal_digest_is_valid` admits raw digest bytes through vendored verified-logic for caller-supplied WOT inputs; `VerifiedWebOfTrustUcanAuthorizationRequest` plus `web_of_trust_ucan_authorization` composes the vendored token, caveat, DID/CID link, threshold trust-chain, and revocation-aware Web-of-Trust UCAN authorizer into an explicit allow/deny decision over caller-supplied public facts. These are focused bridges, not proof resolution, compact-token verification, DID/CID decoding, cryptography, revocation backend/freshness checks, caveat policy evaluation, wildcard containment replacement, networking, persistence, replay checks, or ambient clock behavior.

`CompactToken` owns bearer text in fixed zeroizing storage. It has borrowed `as_str` access, but no `Clone`, `Display`, `FromStr`, or serde implementation. Use `verify_compact_token_str` for borrowed input. The limit check runs before parsing or BLAKE3 identity.

`KeyResolver`, `ProofStore`, `RevocationChecker`, `ReplayAdmission`, `OperationProfileAdmission`, and `HolderInvocationReplayAdmission` are std-shell injection points only. The crate does not provide built-in HTTP, IPFS, database, filesystem, profile registry, replay persistence, or ambient global lookup. `KeyResolutionContext` implements `KeyResolver`, `ProofCollection` implements `ProofStore`, and `RevocationList` implements `RevocationChecker`. `AllowAllReplayAdmission` preserves audience-only compatibility, but it is not holder proof. Holder-signed verification requires an explicit profile observer and replay commit boundary. These hooks remain consumer-agnostic: Aspen, Basalt, or another caller owns resource/action mapping, request canonicalization, identity trust, envelopes, backend freshness, retry, persistence, and outage policy. `VerificationLimits` bounds token and proof verification. `HolderInvocationLimits` adds reviewed field maxima, a 16,384-byte total canonical limit, and fallible exact allocation. See `docs/verified-boundaries.md` for the audit record.

The default-on `durable-authority` feature exposes the concrete durable-state
adapters. Select `default-features = false` to omit that module, its root re-exports,
and its two normal dependency edges. Token and holder verification still require
explicit replay/revocation services; opting out never installs a permissive fallback.
The minimal normal library has a pinned Rust 1.95 check. Full default and dev-test
profiles retain the current owner toolchain and dependency requirements. This is
feature selection, not a storage, freshness, or authority guarantee.

`DurableHolderReplayAdmission` and `DurableRevocationChecker` are shell composition adapters over the separately published Durable Authority State cores. They use exact Radicle source object `a0de79303f3f45c18a34fff535b690a85cb3cf59`. They keep storage ports, time, tenant, generation, authenticity, recovery, and outage policy caller-owned. See [`docs/durable-authority-state.md`](docs/durable-authority-state.md). Resource containment is byte-prefix matching. The exact rule, the shared-prefix overmatch example, and the delimited resource-construction guidance are in [`docs/authorization-reference.md`](docs/authorization-reference.md).

```bash
cargo run --example proof_chain
cargo run --example proof_store
cargo run --example revocation
```

The verified result exposes the issuer, audience, direct capability set, direct proof list, and effective delegation window/capabilities so callers do not need to recompute chain constraints.

Advanced WOT integrations can use `web_of_trust_attestation_payload_bytes`,
`web_of_trust_attestation_signature_is_valid`, `web_of_trust_attestation_is_active_at`,
`web_of_trust_revocation_payload_bytes`, `web_of_trust_revocation_signature_is_valid`,
`web_of_trust_revocation_matches_attestation`, `web_of_trust_revocation_is_effective_at`,
and `web_of_trust_threshold_accepts` for finite public evidence checks. These are
lower-level building blocks beneath `web_of_trust_ucan_authorization`; they do not
resolve identities, fetch evidence, sign records, consult registries, persist state,
or read ambient clocks.

## Protocol profiles

This crate binds every compact token to the nominal `onix-compact-ed25519-v1`
protocol profile. The profile describes the three-part base64url JWT format,
Ed25519 EdDSA signing, JSON claims, and BLAKE3 proof references. It does not
claim final UCAN `1.0.0` compatibility.

Final UCAN `1.0.0` (DAG-CBOR, Varsig, CIDv1) is a declared but unsupported
target profile. `VerifiedToken::protocol_profile()` reports the local profile
for every verified token. See [`docs/protocol-profiles.md`](docs/protocol-profiles.md) for the pinned
source revisions, the fixture manifest, and the bounded conformance report.
AWAKE remains a separate session protocol with no token-format claims here.

## Optional Nostr identity adapter

<!-- r[impl ucan.nostr_identity_adapter.package_boundary] -->
<!-- r[impl ucan.nostr_identity_adapter.support_profile] -->

`ucan-nostr` is an optional workspace package. The root `ucan` and `ucan-core` packages do not depend on it.

The adapter verifies bounded NIP-01 events and explicit NIP-42, NIP-98, or `application-v1` contexts. It uses NIP-required SHA-256 for event IDs and BLAKE3 for adapter identities. The output proves Nostr key control for one bound request only.

Consumers retain relay access, DNS, signer custody, clocks, replay storage, subject resolution, and authorization. See [`docs/nostr-identity-adapter.md`](docs/nostr-identity-adapter.md) for pinned standards, vectors, dependency versions, licenses, and exact non-claims.

## Support limits

This crate is **controlled-integration ready**, not a turnkey production UCAN platform. The machine-readable and operator-facing support report is maintained in [`docs/production-readiness.md`](docs/production-readiness.md) and exposed in Rust through `production_readiness_report()`.

Reviewed Nickel integration profiles under `contracts/integration-profiles/` are authoring-time declarations bound to the Rust support catalog by BLAKE3. They do not run in token or authorization paths. Check catalog, profile, fixture, and documentation freshness with `cargo run --locked --bin check-production-readiness -- check --root . --nickel nickel`.

- Production-oriented issuance should use `TokenSigner` / `issue_token_with_signer`; `IssueRequest` remains as a compatibility wrapper around the in-memory Ed25519 signer path.
- Supported DID methods: `did:key` for Ed25519 public keys; `decode_did_key` is a structural byte decoder only and does not resolve DIDs or verify signatures.
- Supported signing algorithms: Ed25519 compact-token signatures with `alg = "EdDSA"`.
- Proof-loading model: caller-supplied proof tokens only; the library does not fetch proofs from storage or the network.
- Revocation model: caller-supplied `RevocationChecker` values can deny the leaf token or any proof-chain token by `ProofReference`; revocation registries, credential formats, replay stores, persistence, and remote fetches remain application-owned.
- Replay model: the replay admission key is the BLAKE3 digest of the bounded compact token; identical bytes plus an identical (resource, ability) pair are indistinguishable. The default audience-only `AllowAllReplayAdmission` accepts every replay, so exactly-once semantics require a caller-owned store. Holder-signed verification uses `HolderInvocationReplayAdmission` exactly once after token, signature, profile, capability, and caveat checks. Durable nonce truth, request provenance, crash recovery, duplicate semantics, and backend freshness remain application-owned. See [`docs/invocation-reference.md`](docs/invocation-reference.md); the baseline store lives in `examples/replay_baseline.rs`.
- Out-of-scope UCAN features in this release: remote DID resolution, network proof fetching, storage layers, caveat-specific attenuation rules beyond caller policy evaluation, registry-backed revocation, replay stores, and non-Ed25519 key types.

## Development

```bash
nix develop -c cargo test --workspace
nix develop -c cargo fmt --manifest-path Cargo.toml -p ucan -p ucan-core -p ucan-nostr --check
nix develop -c cargo clippy --manifest-path Cargo.toml -p ucan -p ucan-core -p ucan-nostr --all-targets --no-deps -- -D warnings -A clippy::cargo-common-metadata
nix flake check
```

The dev shell also includes correctness tooling for follow-up hardening work:

```bash
cargo-mutants --help
cargo fuzz --help
uv --version
```

Hegel-backed property tests use `uv` on first run to install the pinned `hegel-core` helper into `.hegel/`, which is intentionally gitignored.

`vendor/verified-logic` is a checked-in upstream snapshot. Keep it byte-for-byte aligned with upstream and exclude it from this repo's local `fmt`/`clippy` policy; refresh it with the sync script instead of reformatting or relinting it in place.

Refresh the vendored `verified-logic` snapshot from the sibling Trellis source repo with:

```bash
./scripts/sync-verified-logic.sh ../trellis
```

That script imports the committed `HEAD` revision from Trellis, ignores uncommitted sibling changes, and records the imported revision in `vendor/verified-logic-revision.txt`. The Josh path-selection pilot is documented in `docs/verified-logic-josh-sync.md`; `josh/verified-logic.josh` is the canonical filter for the vendored subtree.

## Optional peer-auth framing

Enable `ucan-core` feature `peer-auth` to sign and verify deterministic message frames. The feature has no clock, transport, key-store, or replay-store effects. See [the peer-auth evidence and boundary](docs/peer-auth-protocol.md).

## Verification

```bash
verus verus/example_spec.rs
```

Install and run the local pre-commit hook for the cheap Josh sync boundary check:

```bash
pre-commit install
pre-commit run verified-logic-josh-sync-config --all-files
```

The hook runs the focused Nix check only; it validates the Josh filter, verifier unit tests, and revision metadata without requiring a sibling Trellis checkout.

Verified replay-admission transcript aggregation gates the audience-only `verify_invocation_with_replay` compatibility path. Holder-signed admission separately verifies the audience key over a bounded canonical request and uses caller-owned profile and replay boundaries. Neither path proves nonce-store truth, durability, downstream request semantics, transport identity, or execution safety.

## License

`ucan-core` and the synchronized `verified-logic` snapshot are `MPL-2.0`; `ucan-nostr` and the root `ucan` std shell are `AGPL-3.0-or-later`. Complete texts and the package/vendor map are in [LICENSES](LICENSES) and [docs/licensing.md](docs/licensing.md).

The snapshot records a committed Trellis source revision and preserves its license artifacts. That alignment does not certify UCAN shell behavior, external protocol interoperability, resolver truth, revocation freshness, replay durability, or whole-system correctness. Earlier grants and third-party terms remain intact.

## References

- [Transactional Reconciliation Core at `eb2bd3441753af97bfcb247cef7cc22d72675b62`](https://seed.radicle.garden/z4Tky6zvC8w4Y6c4YBzNxVbq5n752.git) — exact publication, quarantine, and read-back pilot for durable authority state. UCAN retains replay, revocation, authorization, persistence adapters, and protected-effect authority.
- [Nostr NIPs at `656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab`](https://github.com/nostr-protocol/nips/tree/656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab) — NIP-01, NIP-42, and NIP-98 interoperability reference. UCAN retains token algorithms, DIDs, and authority boundaries.
- [BIP-340 and vectors at `c38071c8c45a1fc50cecaac0d82d99e3bbd56911`](https://github.com/bitcoin/bips/tree/c38071c8c45a1fc50cecaac0d82d99e3bbd56911/bip-0340) — x-only secp256k1 signature and test-vector reference.
- [AeneasVerif/aeneas](https://github.com/AeneasVerif/aeneas) — exact-source Aeneas/Lean proof-pilot reference for selected safe deterministic `ucan-core` functions; any receipt remains property- and source-scoped and does not transfer parser, cryptographic, authorization, interoperability, or whole-system claims.
- [Celld peer authentication at `c978a2590e875b003ecd860a86cb7551287eba67`](https://github.com/devinautotomic226/celld/blob/c978a2590e875b003ecd860a86cb7551287eba67/crates/celld/peer_auth.rs) — bounded `Apache-2.0` version, body-digest, HMAC, clock, and replay comparison source. UCAN does not claim parity, equivalence, or transport support.
