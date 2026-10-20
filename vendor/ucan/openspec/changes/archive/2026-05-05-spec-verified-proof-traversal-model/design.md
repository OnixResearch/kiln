## Context

The shell currently owns resolver/proof-store calls and recursively verifies compact parent proofs. Bridge helpers cover proof-depth admission, proof-link issuer/audience checks, structural chain snapshots, and non-revoked chain snapshots, but not the exact traversal algorithm with backend error behavior.

## Goals / Non-Goals

**Goals:** define a future finite resolved traversal model that can be verified in `verified-logic` and safely consumed by the shell.

**Non-Goals:** no hidden proof fetching, no network/filesystem fallback, no global resolver/store, no cryptographic signature verification inside the traversal predicate, no revocation registry freshness checks, and no automatic compact-token codec verification.

## Decisions

### 1. Model resolved traversal evidence before changing shell behavior

**Choice:** A future implementation MUST first define finite resolved traversal inputs and expected outcomes before routing shell traversal through a verified predicate.

**Rationale:** The existing shell behavior mixes parsing, resolver I/O, signature checks, limit enforcement, and recursion. A verified primitive needs a data-only boundary to avoid proving backend behavior.

**Alternative:** Directly rewrite recursive shell verification around existing structural-chain helpers. Rejected because it would not close the traversal stack/cycle/backend-error gap and could weaken current fail-closed behavior.

## Risks / Trade-offs

**Over-modeling beyond current shell needs** → Keep the first model to finite resolved proof references, parent-child links, depth/count/input budgets, cycle indicators, and backend result classification.
