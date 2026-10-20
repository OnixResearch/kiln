## Why

UCAN currently translates shell proof-hop depth to verified chain length with local `depth.checked_add(1)` before calling `verified_logic::ucan_delegation_depth::check_depth_budget`. That overflow/admission fact is deterministic and belongs beside the existing verified delegation-depth budget primitive.

## What Changes

- Add an executable `verified-logic` helper that accepts shell proof-hop depth and max depth, rejects `usize` overflow, and composes the existing chain-length budget predicate.
- Sync UCAN vendored `verified-logic` and route `proof_depth_within_budget` through the new helper.
- Update docs, bridge coverage, and guard checks so the hop-to-chain-length conversion does not drift back into shell-only logic.

## Non-Goals

- No proof traversal, backend, proof material provenance, revocation, caveat, crypto, parser, clock, or policy truth is verified by this change.
- No public UCAN API shape changes are intended.
