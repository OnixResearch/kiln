# Nostr identity adapter evidence

The optional `ucan-nostr` package verifies key control for one bound request. It does not change UCAN token algorithms, DID methods, proof paths, or authorization.

## Reviewed standards and vectors

The review used these exact sources:

| Source | Revision | Selected material | License evidence |
|---|---|---|---|
| `nostr-protocol/nips` | `656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab` | NIP-01, NIP-42, and NIP-98 | The pinned `README.md` states that all NIPs are public domain. |
| `bitcoin/bips` | `c38071c8c45a1fc50cecaac0d82d99e3bbd56911` | BIP-340 and `bip-0340/test-vectors.csv` | BIP-340 states `BSD-2-Clause`; its code and vectors offer `BSD-2-Clause OR MIT OR CC0-1.0`. Test-vector literals use the CC0-1.0 option. |

The focused test uses BIP-340 vectors 0 and 5. It also creates signed NIP-01 events for each supported authentication profile.

## Dependency evidence

`Cargo.lock` records these exact direct adapter dependencies:

| Package | Version | Registry checksum | License |
|---|---:|---|---|
| `k256` | `0.13.4` | `f6e3919bbaa2945715f0bb6d3934a173d1e9a59ac23767fbaaef277265a7411b` | `Apache-2.0 OR MIT` |
| `sha2` | `0.10.9` | `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283` | `MIT OR Apache-2.0` |
| `blake3` | `1.8.4` | `4d2d5991425dfd0785aed03aedcf0b321d61975c9b5b3689c774a2610ae0b51e` | `CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception` |
| `serde_json` | `1.0.149` | `83fc039473c5595ace860d8c4fafa220ff474b3fc6bfdb4293327f1a37e94d86` | `MIT OR Apache-2.0` |

The adapter uses `k256` BIP-340 verification. It does not implement elliptic-curve arithmetic.

## Checked profile

The adapter applies these rules:

- NIP-01 event identity uses canonical JSON and SHA-256.
- BIP-340 verification uses the raw 32-byte NIP-01 event identity.
- NIP-42 requires kind `22242`, an exact relay URL, and an exact challenge.
- NIP-98 requires kind `27235`, an exact absolute URL, an exact method, and an exact optional payload SHA-256 identity.
- `application-v1` binds domain, audience, purpose, challenge, request identity, and a caller-owned event kind.
- The application profile rejects the NIP-42 and NIP-98 event kinds.
- Freshness uses caller-supplied current time, maximum age, and future skew.
- Replay admission must name the exact verified event identity.
- Adapter identities and request references use domain-separated BLAKE3.

The NIP-42 and NIP-98 profiles require empty content. This is a strict adapter subset.

## Retained responsibilities

Consumers retain relay access, URL policy, DNS, NIP-46 sessions, signer custody, clocks, replay storage, subject resolution, and authorization. A `VerifiedNostrCredential` proves only the checked key-control statement for one request.
