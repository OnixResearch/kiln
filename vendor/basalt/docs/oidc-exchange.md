# OIDC-to-UCAN Exchange

Basalt verifies one GitHub Actions OIDC token and maps accepted public identity
facts to one narrow Mantle UCAN capability. Token acquisition remains in
Lattice or another client shell.

## Policy

`contracts/oidc-exchange/policy.ncl` selects these facts:

- exact GitHub issuer, discovery URL, JWKS URL, and audience;
- RS256 only;
- exact repository owner and repository;
- workflow and Git reference prefixes;
- explicit clock skew, token age, UCAN lifetime, network, body, key, cache, and refresh limits;
- one versioned Mantle capability vocabulary;
- one-time exchange with an issuer-provided `jti`.

The generated policy contains no private key, provider credential, OIDC token,
or secret value.

## Verification order

1. The shell reads the untrusted header only to select `kid`.
2. Bounded HTTP retrieves exact discovery and JWKS URLs.
3. The shell rejects unsafe redirects, issuer drift, excessive documents, and excessive key sets.
4. The pure core fixes RS256 and verifies the signature with the selected public RSA key.
5. The core checks issuer, audience, `exp`, `nbf`, `iat`, repository, workflow, commit, and reference.
6. The core attenuates the requested Mantle audience, project, operation, resource, and lifetime.
7. The shell commits one replay key derived from public facts and the exact request.
8. The caller-owned UCAN signer issues one token through the reviewed UCAN library.

Signature validity does not grant authority by itself.

## Rotation and cache policy

The cache accepts freshness only up to the Nickel policy maximum. An unknown
`kid` can cause one refresh. Duplicate `kid` values fail closed. Stale keys are
not used during an outage.

Operators rotate the issuer key by publishing the new public JWK before tokens
use its `kid`. The old key can remain in the bounded set during the provider
rotation window. Basalt does not extend cache freshness beyond policy.

The UCAN issuer signing key remains behind the SecretSpec boundary. The OIDC
exchange requires an admitted SecretSpec load, the reviewed declaration, and a
public rotation digest that matches the caller-owned `TokenSigner` identity.
It rejects a mismatch before network or replay effects. It does not expose
private key bytes or create ambient key custody.

## Revocation and replay

OIDC public keys are accepted only while the bounded JWKS cache is fresh. A
removed key becomes unusable after the next required refresh. Basalt does not
claim instant provider revocation.

The replay port must atomically reserve the planned key until UCAN expiration.
A duplicate or unavailable store denies issuance. The receipt cannot recreate
replay authority.

## Evidence

Receipts contain policy, issuer-profile, public subject, key, requested
capability, granted capability, outcome, and reason identities. Receipts omit
raw OIDC tokens, UCAN bearer tokens, private keys, provider credentials, full
JWK sets, and token-derived correlation digests.

## Incident response

If issuer, signing, replay, or network evidence becomes unsafe:

1. Disable the exchange endpoint.
2. Deny new exchanges in deployment policy.
3. Rotate the UCAN issuer key through SecretSpec.
4. Remove unsafe OIDC keys at the provider.
5. Expire or revoke issued UCANs through the existing authority system.
6. Preserve metadata-only receipts for review.

Do not log the incoming OIDC token or issued UCAN.

## Rollback

Rollback disables this exchange module and restores the previous Basalt
release. It does not add a static token fallback and does not broaden Mantle
authority.
