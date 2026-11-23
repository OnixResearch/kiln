# Nostr identity bindings

Basalt maps one reviewed UCAN Nostr credential to one policy-owned stable subject. The mapping is authentication evidence. It is not authority.

## Producer boundary

The public adapter accepts only `ucan_nostr::VerifiedNostrCredential` from reviewed Nostr credential revision `c9d43a7c11dd2fab4d5cb3d100012fa9a2e5b026`.

Basalt builds from integrated UCAN revision `fca62a6ed6895689ff18112b497bc7a986516c20`. This revision contains the Nostr adapter, peer authentication, and holder-signed invocation admission.

The adapter does not accept `signature_valid`, `authenticated`, or similar caller booleans.

The published producer identities are:

- schema BLAKE3: `45824d99ae765e885f085ef2c8c8294343415af8e5489c35509c3e255476e258`;
- implementation-profile BLAKE3: `6e0dcbc7d357973695a9476594ff24f7d898e6eed837eb169ebdec0212f39a25`.

`contracts/nostr-identity/generated/identities.json` records these values for downstream pinning. The implementation profile is not a source-code proof.

Regenerate this inspection projection with `cargo run --example nostr_identities --quiet`.
The exporter calls the public Rust identity functions and embeds the checked policy.
Capture stdout to a temporary file before replacing the checked projection. Nickel's
`identity` field alone is not this producer record and lacks its computed digests.

## Policy

`contracts/nostr-identity/policy.ncl` is the typed source. `contracts/nostr-identity/generated/policy.json` is the checked runtime projection.

The reviewed policy BLAKE3 identity is `9f5df806b5fac55a35a63f58c86e4218de1414141c20c999cbd140ea3c0556a2`.

A subject has a stable subject ID and one or more Nostr authenticators. Each authenticator has an `active`, `retired`, or `revoked` state.

Only an active key can produce `AuthenticatedNostrSubject`. A retired or revoked key produces a denied receipt. Key rotation can preserve the subject ID while changing the binding ID and public key.

## Digest roles

Receipts keep these roles separate:

- `nostr_event_sha256` for the external NIP-01 event ID;
- `ucan_adapter_blake3` for UCAN profile, implementation, and request identities;
- `basalt_blake3` for policy, binding, public-field, and receipt identities.

A digest with the wrong role fails validation. Basalt does not replace a Nostr event ID with a BLAKE3 rehash.

## Authorization composition

`compose_nostr_authorization` accepts an opaque authenticated subject and one separate authorization observation.

A composed allow requires the authorization to be current and allowed. Subject, request, and policy identities must match exactly. Missing, denied, stale, or mismatched authorization remains denied.

Authentication and composition receipts always use `recorded_only`. They do not count as property evidence or live authority.

## Privacy

The closed policy and receipt DTOs reject unknown fields. The boundary excludes private keys, credentials, proof bytes, challenges, NIP-46 secrets, signer secrets, and encrypted payloads.

Receipts retain public keys and bounded public identities. They hash audience and purpose text before output.

This validation does not prove that arbitrary public text is secret-free. Producers and callers must still apply their own disclosure policy.

## Non-claims

Basalt does not prove a human identity, key custody freshness, relay completeness, NIP-05 truth, policy publication, replay-store durability, or release eligibility.

The caller retains clocks, replay storage, policy distribution, authorization, resource and ability meaning, deployment, and release decisions.

BLAKE3 receipt identities detect changed bytes under the declared algorithm. They do not authenticate a producer or establish trust.
