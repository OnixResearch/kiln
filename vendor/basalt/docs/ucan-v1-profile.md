# Final UCAN 1.0 application adapter

Basalt accepts the final `ucan-wg-1.0.0` profile through a separate adapter. The compact adapter remains available and keeps its existing behavior.

## Source cohort

The adapter binds UCAN revision `417f74662b9de2de3bdabe93ad5db996e1bd2861`.

`contracts/ucan-v1-profile/source-evidence.ncl` records these reviewed inputs:

- archived wire-profile evidence and specification identities
- archived authorization evidence and specification identities
- the support-catalog BLAKE3 identity
- the vendored file-manifest BLAKE3 identity
- preserved license expressions
- supported and tested final-profile status

Missing, stale, failed, untested, mismatched, or incomplete source evidence fails closed.

## Authority flow

```text
final Invocation envelope
  -> UCAN wire, signature, proof, policy, time, and revocation checks
  -> Basalt replay gate receives verified command and subject facts
  -> closed Nickel-exported command mapping
  -> caller-owned exact resource-owner observation
  -> Basalt application policy
  -> prebuilt redacted receipt and prospective live authority
  -> one caller-owned final replay commit
  -> UCAN live authorization and private Basalt live authority
```

The Basalt replay gate wraps UCAN's caller-owned replay port. Mapping, owner checks, policy checks, allocation, and receipt construction finish before the wrapped port runs.

A Basalt denial makes zero calls to the wrapped replay port. UCAN calls the wrapper at most once. Basalt does not make a second replay call.

## Closed mapping

`contracts/ucan-v1-profile/mapping.ncl` maps exact slash commands to existing `EnforcementAction` values.

Every mapping requires these facts:

- the exact final protocol profile
- the exact UCAN source cohort
- one exact command
- the exact UCAN subject
- a caller-owned owner observation for the exact Basalt resource
- one existing Basalt contract, ability, and operation profile

Unknown, duplicate, incomplete, lossy, or cross-profile mappings deny.

## Live and persisted values

`LiveFinalProfileEnforcement` contains UCAN's private-construction `LiveAuthorization`. It borrows the exact Invocation envelope and Basalt request.

`FinalProfileReceipt` contains metadata identities and closed outcome classes. It omits raw envelopes, proofs, arguments, subjects, resources, policies, keys, and credentials.

A receipt is evidence only. Deserializing it cannot construct live authority.

## Retained boundaries

UCAN owns wire, signature, proof-chain, command, policy, revocation-observation, and replay semantics.

Basalt owns the application command mapping and contract policy.

Consumers retain these responsibilities:

- key trust and key custody
- external resource ownership
- clock and revocation freshness
- replay durability and recovery
- transport security
- policy completeness
- effect execution and correctness
- release eligibility
- whole-system safety

An allow applies only to the exact checked cohort, mapping, request, and replay attempt.

<!-- r[impl basalt.ucan_token_flow.v1_profile.docs] -->
