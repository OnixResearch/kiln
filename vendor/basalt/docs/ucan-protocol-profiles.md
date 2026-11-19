# UCAN protocol profiles

Basalt binds each UCAN authority input to one explicit nominal protocol profile.
It does not infer a profile from credential bytes.

## Current cohorts

- Profiles: `onix-compact-ed25519-v1` and `ucan-wg-1.0.0`
- Vendored UCAN revision: `417f74662b9de2de3bdabe93ad5db996e1bd2861`
- Support catalog BLAKE3: `fe77fd63076eaef555b74e702e4c14e483bdba70151ccc955c75b17d97412427`
- Basalt policy BLAKE3: `fd17db568eb24222d4ca65fe45f02edb21ef910fad214a392441d62c07ef4b14`

The compact Nickel cohort is `contracts/ucan-profiles/cohort.ncl`.
The final Nickel cohort is `contracts/ucan-profiles/final-cohort.ncl`.
They reject unknown profiles, stale source revisions, adapter substitution, and weakened non-claims.

## Adapter selection

`UcanEnforcementRequest` requires a `BasaltUcanProtocolProfile`.
The existing compact adapter accepts only `OnixCompactEd25519V1`.
It rejects `UcanWg100` before token parsing and before replay state is consumed.
It does not inspect bytes and select another decoder.

`VerifiedUcanAuthorityInput` is the Basalt-owned application contract.
Only the matching compact or final adapter can construct it.
The value binds the profile, source cohort, subject, operation profile, request, credential identity, grant class, and non-claims.
It contains no UCAN wire or codec type.

The final adapter uses UCAN's final-profile shell and a separate Basalt replay gate.
Read [`ucan-v1-profile.md`](ucan-v1-profile.md) for its closed mapping and ordering rules.
Profile matching alone does not prove final-profile compatibility.

## Receipts

UCAN enforcement receipt schema `basalt.ucan-enforcement-receipt.v2` binds these values:

- protocol profile;
- vendored UCAN revision;
- UCAN support-catalog identity;
- Basalt policy identity;
- operation profile;
- bounded token and request identities;
- decision reason and non-claims.

A receipt is recorded evidence only.
It cannot recreate `LiveUcanEnforcement` or grant authority.

## Non-claims

Profile matching does not prove token correctness, key trust, revocation freshness, replay durability, policy completeness, transport security, or effect correctness.
UCAN retains protocol wire and authority semantics.
Basalt retains application policy and action mapping.
Callers retain external state, clocks, credentials, transport, and effects.
