# UCAN 1.0.0 Authorization

The final profile is available through:

- `ucan_core::profile::v1_authorization` for deterministic command, policy,
  chain, and admission rules.
- `ucan::profile::v1_authorization` for wire verification, CID-bound proof
  loading, replay orchestration, live authorization, and receipts.

This profile is separate from `onix-compact-ed25519-v1`. It does not reuse the
compact wildcard ability rules.

## Admission order

`authorize_invocation` uses this fixed order:

1. Parse and verify the final-profile Invocation envelope.
2. Load each Delegation in declared root-to-invoker order.
3. Enforce per-proof and total byte limits.
4. Recompute each Delegation CID and compare it with the requested CID.
5. Resolve each Delegation issuer key and verify its signature.
6. Observe revocation for each proof CID.
7. Check principal alignment, subject continuity, executor identity, time,
   command attenuation, policy, proof identity, and work limits.
8. Call the caller-owned replay port once.
9. Construct `LiveAuthorization` without another fallible operation.

A failure before step 8 makes zero replay calls. A replay denial makes one
replay call and returns a denied outcome.

## Commands and policy

Final commands start with `/`. They have no empty or trailing segment. A
parent command covers itself and complete descendant segments. For example,
`/crud` covers `/crud/create`. It does not cover `/cruddy`.

The policy parser accepts only the pinned final-profile operators:

- `==`, `!=`, `<`, `>`, `<=`, `>=`, and `like`
- `and`, `or`, and `not`
- `any` and `all` over list or map values
- dotted selectors and `.` for the current quantified value

Unknown operators and unsupported jq features fail closed. Unresolved required
selectors evaluate false. Depth, statement, and work limits are explicit.

## Caller-owned ports

The caller supplies:

- `DidKeyResolver` for issuer-key truth
- `V1DelegationStore` for CID-addressed proof bytes
- `V1RevocationObservation` for current revocation facts
- the exact expected executor
- verification time and clock-skew policy
- `V1InvocationReplay` for replay admission and persistence policy

Replay failures use closed classes: duplicate, stale, malformed, denied, or
unavailable. Backend text cannot enter a receipt.

No ambient network, storage, clock, registry, nonce source, or key custody is
installed.

A store result is not trusted by location. The shell recomputes the returned
Delegation CID and compares it with the requested CID before authority use.

## Live authorization

`LiveAuthorization` has private fields. It borrows the exact Invocation
envelope. It is not `Clone` and has no serde implementation or public
constructor.

The value exposes only admitted facts, such as the subject, executor, root
issuer, command, Invocation CID, and exact envelope borrow. A receipt cannot
recreate this value.

## Receipts

`AuthorizationReceipt` is deterministic and redacted. `receipt_blake3`
computes a domain-bound BLAKE3 identity over canonical receipt bytes.

A receipt includes profile and cohort identities, Invocation and proof CIDs,
BLAKE3-hashed command, subject, executor, and root-issuer identities, replay
and revocation classes, limits, the result class, and non-claims. It does not
include raw bearer credentials, Delegation envelopes, Invocation arguments,
metadata, policy payloads, or backend error text.

## Conformance boundary

All pinned Delegation, Invocation, and policy rows pass. The upstream
`policy.json` has one malformed anonymous object wrapper. The report generator
keeps the pinned bytes unchanged and applies one exact in-memory normalization
before policy execution. See `docs/protocol-profiles.md`.

Passing admission does not prove transport security, global freshness, durable
replay storage, external resource ownership, policy completeness, backend
truth, or effect correctness.
