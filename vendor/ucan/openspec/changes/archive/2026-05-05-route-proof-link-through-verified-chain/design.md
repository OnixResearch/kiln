## Context

`verified-logic::ucan_chain::check_chain_linked` verifies that each resolved child issuer equals its parent audience. The UCAN shell already validates signatures, proof-store resolution, revocation, capability attenuation, and temporal bounds around that link.

## Decisions

### Bridge a two-entry chain for link checks

**Choice:** Introduce `proof_link_is_valid(parent_audience, child_issuer)` in `src/verified.rs`. It adapts the two public DIDs into a minimal two-entry `ucan_chain::ChainEntry` vector and calls `check_chain_linked`.

**Rationale:** This keeps proof-link semantics in the verified bridge without changing the shell's resolver/proof-store shape or pretending the bridge performs cryptography, revocation freshness, or proof fetching.

**Rejected:** Replacing the whole recursive verifier with `check_chain_structural_valid`; the shell supports caller-injected proof stores, revocation backends, caveats, and a multi-parent effective delegation model that need a larger design.

## Risks / Trade-offs

- The helper is intentionally structural: it proves only issuer/audience linkage over caller-provided DIDs.
- Guard script patterns must be narrow enough to prevent the old direct comparison while allowing legitimate DID accessors.
