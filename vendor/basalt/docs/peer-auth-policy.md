# Peer authentication policy boundary

Basalt evaluates application policy over `ucan-core`'s opaque `VerifiedAuthenticatedMessage` value.

The verdict binds a protocol version, message BLAKE3 identity, body-bound authentication reference, freshness result, replay result, and implementation identity.

Basalt does not parse the peer message or verify its HMAC.

## Flow

```text
consumer transport and key custody
  -> ucan-core authenticated-message verification
  -> opaque successful verdict
  -> reviewed Basalt Nickel policy
  -> pure Basalt peer-action decision
  -> metadata-only BLAKE3 receipt
```

The pure Basalt core reads no clock, key store, replay store, transport, filesystem, process, or environment.

## Policy

`contracts/peer-auth/policy.ncl` declares every required fact:

- protocol version `1`
- freshness `within-clock-bound`
- replay `fresh`
- action `dispatch-control-message`
- authority `authenticated-peer-message`
- default decision `deny`
- exact UCAN source revision
- bounded Celld comparison reference
- consumer-owned non-claims

Missing facts and unknown fields fail closed. No default fact is inferred.

## Celld reference

The bounded comparison source is Celld revision `c978a2590e875b003ecd860a86cb7551287eba67`.

The selected source blob is `40d6ac4613d293a148e7222e192153eaeaeb29db`. Its recorded license is `Apache-2.0`.

This reference supplies version, body binding, clock, HMAC, and replay comparison constraints. Basalt does not claim parity or equivalence.

## Decisions and receipts

Only an opaque UCAN verdict and exact Basalt policy can construct `PeerAuthDecision`.

The receipt includes public BLAKE3 identities and closed outcome classes. It excludes message bodies, routing contexts, keys, authentication tags, and end-user identity facts.

The decision authenticates one peer message for one bound action. It does not grant a UCAN capability or general authority.

## Consumer responsibilities

Consumers retain:

- transport and TLS
- key custody and rotation
- clock observations
- durable replay storage
- membership and routing
- end-user authentication
- authorization and effect execution

The JSON cases in `contracts/peer-auth/generated/steel-fixtures.json` form the bounded Steel adapter matrix.
