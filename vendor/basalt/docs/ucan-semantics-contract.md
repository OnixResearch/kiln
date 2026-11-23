# UCAN semantics compatibility contract

Basalt's UCAN semantics fixtures test one bounded integration claim: decision facts supplied by a UCAN integration are rejected when any required fact is non-admitted, and only admitted effective grants are replayed through Basalt's existing Nickel-authored enforcement core.

## Identity boundary

Every fixture binds five immutable identities:

- the admitted UCAN protocol profile: `onix-compact-ed25519-v1`
- Basalt's reviewed vendored UCAN revision: `fca62a6ed6895689ff18112b497bc7a986516c20`
- the reviewed UCAN support-catalog revision: `fca62a6ed6895689ff18112b497bc7a986516c20`
- that catalog's BLAKE3 identity: `fe77fd63076eaef555b74e702e4c14e483bdba70151ccc955c75b17d97412427`
- the generated Basalt policy artifact's BLAKE3 identity: `e929bde8e316da188eedb55013a695e82c280c69b88c15d0668c119589beb233`

The UCAN runtime and support catalog come from the same checked-in snapshot.
Basalt does not claim that the unpushed source revision is available from a remote source.

## Fixture and adapter flow

1. Human-reviewed fixtures are authored in Nickel under `contracts/ucan-semantics/fixtures/` and checked against `contracts/ucan-semantics/contracts.ncl`.
2. Nickel exports deterministic JSON into `fixtures/generated/`; the Nix freshness check rejects source/artifact drift and verifies that malformed contract fixtures cannot export.
3. `basalt::ucan_semantics` validates schema, dependency identity, policy identity, bounded capability counts, UCAN capability syntax, expectations, and non-claims.
4. The adapter rejects wildcard requests and any non-admitted delegation, caveat, proof-traversal, or replay fact.
5. Only admitted direct and effective delegated grants become an `EnforcementRequest` passed to the existing `basalt::enforce` function.

The adapter consumes UCAN decisions as data. It does not parse tokens, validate signatures, derive effective delegation, load proofs, resolve keys, evaluate caveats, inspect clocks, or own replay storage.

## Covered fixtures

| Fixture | Expected result |
|---|---|
| `positive/allow.ncl` | Matching policy and capability allow |
| `positive/capability-deny.ncl` | Matching policy with non-authorizing capability denies |
| `negative/over-delegation.ncl` | UCAN over-delegation fact denies before enforcement |
| `negative/wildcard-request.ncl` | Non-concrete operation request denies |
| `negative/missing-caveat-policy.ncl` | Missing caveat policy denies |
| `negative/stale-proof.ncl` | Stale proof traversal denies |
| `negative/replay-denied.ncl` | Replay backend failure denies |
| `malformed/missing-operation.ncl` | Nickel contract rejects empty operation |
| `malformed/weakened-non-claims.ncl` | Nickel contract rejects removed claim boundary |
| `malformed/incompatible-expected-result.ncl` | Nickel contract rejects an allow expectation for denied facts |
| `malformed/stale-catalog.ncl` | Nickel contract rejects catalog identity drift |

## Explicit non-claims

These fixtures and replay reports do **not** prove:

- revocation-backend freshness
- key-resolution truth
- UCAN cryptographic correctness
- consumer replay-store durability
- application-level authorization completeness

UCAN or its caller remains authoritative for token cryptography, proof and caveat facts, explicit verification time, revocation inputs, replay state, and key resolution. Applications remain authoritative for the completeness of their policy and for acting on Basalt's receipt.

## Validation

Run the fixture freshness and replay checks from the repository root:

```bash
nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).ucan-semantics-fixtures --no-link
nix develop -c env CARGO_TARGET_DIR=/tmp/basalt-semantics-target RUSTC_WRAPPER= CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --locked --test ucan_semantics_contract --no-default-features
```

The direct release verifier and `nix flake check -L` include these rails. Package-content validation also requires the Nickel contract, generated fixtures, adapter module, and integration test.
