# Durable Authority State pilot

Basalt is the second checked consumer of Durable Authority State.
It uses the same exact source as UCAN.

- Canonical RID: `rad:z3kGDT73ja8rewvGLwJFLF5MTx1ih`
- Admitted adapter: `https://seed.radicle.garden/z3kGDT73ja8rewvGLwJFLF5MTx1ih.git`
- Reviewed object: `a0de79303f3f45c18a34fff535b690a85cb3cf59`
- Packages: `replay-ledger-core` and `revocation-view-core`

Cargo declares both packages directly.
The lockfile and Nix source check bind the same full object.
There is no GitHub, sibling, branch, tag, or path fallback.

## Composition

`BasaltDurableAuthority` composes UCAN's published `DurableHolderReplayAdmission` and `DurableRevocationChecker`.
It does not copy replay or revocation decisions.

The caller supplies these values:

- one `ReplayLedgerPort`;
- one `RevocationViewPort`;
- complete replay and revocation contexts;
- keys, proofs, caveat policy, explicit time, and limits.

The helper returns Basalt's existing `UcanEnforcementBoundaries`.
The compact profile check, token verification, action mapping, and Basalt policy run before durable replay.
UCAN invokes replay once as the final state-consuming boundary.

## Authority ownership

Basalt owns application policy, contracts, actions, operation mappings, and live capability construction.
UCAN owns compact protocol parsing, signatures, proof traversal, capability and caveat semantics, holder proof, and replay-call ordering.

Callers own these facts and effects:

- tenant and ledger namespace;
- authority and revocation generations;
- persistence ports and transactions;
- time and revocation authenticity observations;
- reservation recovery and terminal effect outcomes;
- outage policy and protected effects.

## Bounded claim

The pilot proves exact-source composition and the tested positive and negative mappings.
It does not prove global replay exclusion, revocation truth, backend honesty, effect completion, indefinite Radicle availability, final UCAN `1.0.0` compatibility, or release eligibility.
