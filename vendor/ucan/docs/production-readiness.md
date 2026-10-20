# Production Readiness

This crate is **controlled-integration ready**, not a turnkey production UCAN platform.

A controlled integration is one where the consuming application wires all external truth explicitly: key resolution, proof lookup, revocation, verification time, verification limits, caveat policy, signer custody, and replay/persistence policy. The crate supplies deterministic Rust APIs and fail-closed boundaries for those inputs; it does not install ambient network, storage, registry, clock, or key-custody behavior.

The machine-readable support report lives at [`docs/production-readiness.json`](production-readiness.json). The Rust API exposes the same classification through `production_readiness_report()`.

## Rust-owned support catalog

The section below is generated structurally from `production_readiness_report()`. Its BLAKE3 identity also binds the machine JSON, generated Nickel vocabulary, and reviewed profiles. No current surface is `production-supported`; stronger labels require explicit positive, negative fail-closed, operator, and migration-review evidence.

<!-- BEGIN GENERATED UCAN SUPPORT CATALOG -->

Catalog BLAKE3: `2bd91f2d23540781ded43e409130bb23dd77785ac641a33394f8514edd2c6f25`

Catalog schema: `ucan.support-catalog.v1`

| Surface ID | Label | Evidence | Retained boundary |
| --- | --- | --- | --- |
| `ed25519-did-key-compact-token` | `controlled-integration-ready` | issue/parse/verify APIs, signer abstraction, examples, tests, and nix flake checks cover Ed25519 did:key compact tokens | caller owns key custody, key provenance, request policy, and deployment hardening |
| `ucan-wg-1.0.0-authorization` | `controlled-integration-ready` | pinned final-profile wire fixtures, command and policy matrices, ordered CID-bound Delegation chains, exact executor admission, one replay commit, private live values, and redacted receipt tests pass | callers own key truth, proof transport and availability, executor identity, clock truth, revocation freshness, replay durability, policy completeness, transport security, resource ownership, and effect correctness |
| `caller-supplied-key-resolution` | `controlled-integration-ready` | KeyResolver injection and KeyResolutionContext support deterministic local resolution; backend errors fail closed | remote DID method support, trust roots, freshness, and cache behavior are application-owned |
| `caller-supplied-proof-store` | `controlled-integration-ready` | ProofStore injection, ProofCollection, bounded traversal, proof-link checks, and cycle/limit failures are tested | network, filesystem, database, IPFS, content-addressed freshness, and proof provenance are application-owned |
| `caller-supplied-revocation` | `controlled-integration-ready` | RevocationChecker injection, NoRevocations, RevocationList, and leaf/proof-chain denial failures are covered | registry truth, persistence, duplicate semantics, freshness, and backend availability are application-owned |
| `generic-consumer-integration-hooks` | `controlled-integration-ready` | generic key, proof, revocation, profile, caveat, and replay hooks are public and tested without consumer-specific adapters | consumer-specific resource, ability, request, identity, envelope, backend, freshness, retry, persistence, and execution semantics are application-owned |
| `caller-supplied-replay-admission` | `controlled-integration-ready` | ReplayAdmission and verify_invocation_with_replay allow caller-owned duplicate, stale, malformed, denial, and backend policies to fail closed | durable replay detection, crash recovery, backend corruption handling, and cross-process freshness are application-owned |
| `holder-signed-invocation-admission` | `controlled-integration-ready` | borrowed direct and delegated tests bind the complete token reference, verified audience, resource, ability, operation profile, request digest, and fixed nonce to an audience-key signature before one final replay commit | profile registry truth, request canonicalizer provenance, nonce durability, transport identity, action semantics, consumptive rate limits, and execution effects remain application-owned |
| `verified-replay-admission-transcript` | `controlled-integration-ready` | ReplayAdmission outcomes are routed through a vendored verified finite transcript that accepts only allowed outcomes and reports the first duplicate, stale, malformed, denied, backend, or unknown failure | nonce uniqueness truth, request identity provenance, durable replay storage, crash consistency, backend freshness, and consumer envelope semantics are application-owned |
| `verified-proof-traversal-orchestration` | `controlled-integration-ready` | Proof traversal budget, cycle/visited-reference, proof-link, and effective-delegation transcript facts are routed through vendored verified finite traversal predicates before delegated authorization is accepted | proof-store truth, parser output, signature checks, revocation freshness, backend provenance, and runtime recursion remain caller/runtime-owned boundaries |
| `verified-compact-token-codec-structure` | `controlled-integration-ready` | Compact segment framing, exact signing-input/compact reconstruction, base64url segment admission, and decoded JSON object-envelope index discovery route through vendored verified structural predicates | base64url decoding, serde JSON semantics, duplicate-key behavior, canonicalization, UTF-8/string handling, and Ed25519 correctness remain runtime boundaries |
| `verified-core-structural-validators` | `controlled-integration-ready` | DID claim shape, required-claim presence, temporal order, proof-reference/capability/caveat list bounds, and parsed caveat payload shape route through vendored verified finite validators | serde extraction truth, runtime parser behavior, DID method semantics, clock truth, proof/backend truth, and consumer caveat policy remain outside the structural claim |
| `verified-ability-helper-predicates` | `controlled-integration-ready` | VerifiedCapability wildcard-segment and concrete-ability helpers route full-segment wildcard classification through vendored verified ability predicates | consumer-specific resource/action/session policy and mapping for Aspen, Clankers, or other integrations remain downstream-owned |
| `verified-backend-transcript-aggregation` | `controlled-integration-ready` | Key-resolution, proof-load, revocation-check, and replay-admission backend operation facts route through vendored verified transcript aggregation with first-failure semantics | actual key material, proof bytes, registry state, nonce-store freshness, storage durability, network availability, and backend authorization semantics are caller-owned |
| `opaque-caveat-policy-evaluation` | `controlled-integration-ready` | CaveatPolicySet requires every caveat to have an explicit non-consuming policy observation; missing or rejected policies deny authorization | arbitrary caveat semantics, clocks, counters, consumptive quota or rate-limit effects, and external policy truth are application-owned |
| `finite-typed-caveat-helpers` | `preview` | verified finite time-bound and rate-limit helper predicates are exposed for advanced callers | adopting those helpers as a production vocabulary requires integration-specific policy and provenance evidence |
| `web-of-trust-helpers` | `preview` | finite WOT evidence, threshold, and UCAN authorization helpers are exposed below the compact-token shell | DID/CID decoding, evidence fetching, trust-store discovery, revocation freshness, and persistence are not supplied |
| `nostr-identity-adapter` | `preview` | the optional ucan-nostr package verifies bounded NIP-01, NIP-42, NIP-98, and application challenge facts with exact digest roles | relay access, DNS, signing, clocks, replay storage, subject resolution, and authorization remain caller-owned |
| `remote-did-resolution` | `unsupported` | no built-in remote resolver is provided | integrators must supply resolver trust, method admission, freshness, cache, and fail-closed behavior |
| `proof-transport-backends` | `unsupported` | no built-in network, filesystem, database, IPFS, or content-addressed proof transport is provided | integrators must supply proof provenance, freshness, bounds, and reference matching |
| `registry-backed-revocation` | `unsupported` | no built-in registry client or persistence layer is provided | integrators must supply registry truth, freshness, duplicate handling, and outage behavior |
| `replay-persistence-store` | `unsupported` | receipt validation helpers do not store receipts or reject duplicate invocations across restarts | integrators must supply durable replay detection, recovery, corruption handling, and freshness policy |
| `non-ed25519-key-algorithms` | `unsupported` | compact-token signing and verification admit Ed25519 did:key with EdDSA only | future algorithms need parsing, admission, mismatch, signature, and unsupported-algorithm evidence |

### DID methods

| Option | Label | Retained boundary |
| --- | --- | --- |
| `did:key` | `controlled-integration-ready` | key provenance, custody, trust roots, and resolver freshness remain caller-owned |
| `remote-did-method` | `unsupported` | method admission, network resolution, trust roots, caching, and freshness remain caller-owned |

### Signature algorithms

| Option | Label | Retained boundary |
| --- | --- | --- |
| `EdDSA` | `controlled-integration-ready` | key custody, key provenance, and deployment hardening remain caller-owned |
| `non-EdDSA` | `unsupported` | future algorithms require explicit parsing, mismatch, signature, and failure evidence |

### Key classes

| Option | Label | Retained boundary |
| --- | --- | --- |
| `Ed25519` | `controlled-integration-ready` | secret material storage, rotation, provenance, and custody remain caller-owned |
| `non-Ed25519` | `unsupported` | future key classes require explicit format, algorithm, mismatch, and failure evidence |

### Generated verification ceiling

`ucan-default-ceilings`: proof depth `16`, proof count `64`, compact-token bytes `65536`.

### Required non-claims

- Profile conformance does not prove resolver truth.
- Profile conformance does not prove key custody or cryptographic correctness.
- Profile conformance does not prove revocation or replay freshness.
- Profile conformance does not prove backend durability.
- Profile conformance does not prove application authorization completeness.

<!-- END GENERATED UCAN SUPPORT CATALOG -->

## Reviewed integration support profiles

Human-authored profiles live under `contracts/integration-profiles/profiles/`. They select a subset of the Rust catalog, explicitly admit preview surfaces, name finite verification limits, and require caller-owned key resolution, proof loading, revocation, replay, verification time, caveat policy, signer custody, and fail-closed backend behavior.

Run the explicit authoring-time checker with:

```bash
cargo run --locked --bin check-production-readiness -- check --root . --nickel nickel
```

Regenerate the Rust-owned projections after an intentional catalog change, review the new BLAKE3 identity, then update the human-authored profile binding:

```bash
cargo run --locked --bin check-production-readiness -- generate --root .
```

Nickel is never discovered or evaluated by token issuance, parsing, verification, invocation, caveat, revocation, replay, or authorization APIs. Applications may consume a checked export in their own shell, but UCAN runtime decisions continue to receive explicit typed Rust inputs.

For Aspen, this means implementing Aspen-owned resource, ability, identity, revocation, replay, and RPC mappings. Other consumers likewise own their application policy and backend semantics; no consumer-specific adapter belongs in this crate.

## Dependency advisory posture

Assessment date: 2026-08-16. Result: zero exploitable vulnerabilities in the
locked dependency set at the assessment revision.

Two applicable advisories are pinned with explicit non-exploitable rationale
in `deny.toml`: `RUSTSEC-2024-0436` (paste, dev-only via
hegeltest) and `RUSTSEC-2026-0190` (anyhow `downcast_mut`, vendored
`repo_index` module). The `dependency-posture` Nix check enforces the policy
against the committed `Cargo.lock`; it never resolves a fresh dependency set.
See `docs/dependency-posture.md` for the full audit record and non-claims.
