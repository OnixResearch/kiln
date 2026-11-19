# Observe an existing holder authorization

Use `recheck_holder_authorization` when an operation needs another observation
of an existing `HolderInvocationAuthorized`. Do not repeat holder admission.
That operation spends a nonce and cannot serve as a read-only freshness check.

## Inputs and order

1. Retain the original live holder authorization and the exact compact token.
2. Supply current time, verification limits, and trusted key, proof, and revocation services.
3. Supply non-consuming profile and caveat policies for the original operation.
4. Call the recheck. On failure, stop or quarantine the affected work.

The recheck rejects excessive token size before hashing. The pure core then
checks token-digest equality and rejects time before initial verification.
These denials precede all external service calls.

The existing token verifier checks signatures, required proofs, time windows,
and revocation. The recheck then observes the original profile, resource, ability,
and caveats through the existing authorization path. That path selects its first
matching effective capability before evaluating caveats; denial does not select
a different capability. Token and proof digest binding retain the original chain.

## Result

`HolderAuthorizationObservation` borrows the original authorization and records
the supplied observation time. It cannot be constructed publicly, cloned,
serialized, deserialized, or converted into another holder authorization.
The recheck accepts no replay port. It cannot reserve another nonce or start work.

The observation does not extend the effective token bounds. UCAN's existing
inclusive time rules and caller-selected skew tolerance still apply. A consumer
with an exclusive expiry deadline must enforce that stricter rule itself.
The API rejects time before initial admission, not time before every previous
recheck. The consumer must supply monotonic time across successive observations.

## Boundaries

This API trusts the supplied service observations. It does not establish their
authenticity, freshness, persistence, or availability. It does not prove continuing
holder key custody after the original signed invocation. Backend state can change
after success, even while the borrowed result remains live.

A consumer must recheck current authority at its atomic effect boundary. Do not
turn this result into a persistent permission or a resource-release receipt.
Independent custody, workload fencing, cancellation, recovery, and observed
release remain consumer responsibilities. Preserve nonce and invocation records
after release or failure.

Default and `--no-default-features` builds use the same recheck. No permissive
replay or revocation service is installed when durable adapters are omitted.
