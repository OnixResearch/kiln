# OIDC Dependency Review

Basalt uses two immutable dependencies for the OIDC exchange.

## Selected sources

Bounded HTTP supplies HTTPS discovery and JWKS retrieval.

- Radicle RID: `rad:z4ARxEt93T6gJTrTZpUyKHMAemPiv`
- adapter: `https://seed.radicle.garden/z4ARxEt93T6gJTrTZpUyKHMAemPiv.git`
- reviewed object: `5abc2b93e0f288aba251a0750ce2d716eb9757fc`
- license: `MIT OR Apache-2.0`

`ring` supplies the RS256 signature primitive. Basalt keeps compact-JWS
splitting, JWK RSA modulus/exponent decoding, header admission, claim decoding
and every policy decision in its own pure core; `ring` verifies one signature
over the exact `header.payload` bytes.

- version: `0.17.14`
- crates.io checksum: `a4689e6c2294d81e88dc6261c768b63bc4fcdb852be6d1352498b114f61383b7`
- license: `ISC AND MIT AND Apache-2.0`
- selected primitive: `RSA_PKCS1_2048_8192_SHA256`

Basalt fixes the accepted algorithm to RS256 before signature validation, and
rejects any header field outside `alg`, `typ` and `kid`. The earlier
`jsonwebtoken` 11.0.0 dependency is retained only as a dev-dependency to mint
fixture tokens; it is not in the runtime or consumer closure.

## Candidate review

The review used three correlated, serial passes. No subagent performed an
independent review.

| Family | Mechanism | Result |
|---|---|---|
| `ring` RS256 primitive with a first-party compact-JWS core | One audited signature primitive; parsing and policy stay in Basalt | Selected |
| `jsonwebtoken` 11.0.0 | Stable JWT, JWS, and JWK types with a RustCrypto provider | Removed from the runtime closure: its `rsa` 0.9 dependency carries RUSTSEC-2023-0071 and is pinned transitively |
| `jwt-compact` | Typed algorithm API and pure-Rust RSA | Rejected because the reviewed release is beta |
| `josekit` | Broad JOSE implementation through OpenSSL | Rejected because it expands the native and algorithm boundary |

The selected crate does not own OIDC policy. Basalt disables its ambient time
validation and applies issuer, audience, `exp`, `nbf`, `iat`, claim, and
capability policy with explicit time in the pure core.

## Adversarial audit

The tests reject these cases:

- algorithm switching and unsupported token types;
- embedded JWK, JWK URL, X.509, critical, and unknown header fields;
- bad signatures and wrong public keys;
- encryption-use, wrong-algorithm, malformed, duplicate, excessive, and unknown JWKs;
- wrong issuer, audience, time, claim type, repository, workflow, commit, and reference;
- broad audience, operation, project, resource, and lifetime requests;
- DNS, TLS, redirect, timeout, body-limit, parse, cache, rotation, and outage failures;
- replay duplicates and unavailable replay storage;
- raw bearer, private-key, provider-credential, and token-digest leakage.

## Claim boundary

Source identity and test success do not prove cryptographic correctness,
provider honesty, network truth, remote availability, deployment trust,
replay-store durability, or release eligibility.
