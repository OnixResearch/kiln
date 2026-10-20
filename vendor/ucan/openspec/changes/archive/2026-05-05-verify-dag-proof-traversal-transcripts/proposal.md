## Why

UCAN live verification can traverse proof DAGs with multiple direct proofs, but the verified transcript bridge only covers no-proof and single-parent linear chains. Multi-parent traversal remains a high-ROI unverified shell seam after linear and wildcard transcript integration.

## What Changes

- Add a finite verified-logic proof DAG transcript primitive for caller-supplied, topologically ordered traversal data.
- Sync the primitive into UCAN and expose a typed bridge API.
- Keep live token verification boundaries unchanged unless a DAG can be represented deterministically without resolver/proof-store/revocation side effects.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, docs, bridge guard scripts, and tests.
- **APIs**: Adds explicit DAG transcript request/validation helpers; existing resolver/proof-store/revocation APIs remain synchronous and caller-injected.
- **Testing**: verified-logic proof/test gates, UCAN cargo/Nix checks, bridge guard, and OpenSpec validation.
