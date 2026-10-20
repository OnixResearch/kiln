## Context

The existing live proof transcript stores a linear list of entries and can only represent no-proof and single-parent chains. The public DAG bridge accepts a topologically ordered finite transcript with explicit parent indices and checks budgets, visited references, edge well-formedness, temporal validity, revocation flags, and final authorization.

## Goals / Non-Goals

**Goals:**
- Build a deterministic shell transcript from successfully verified parent tokens and the leaf token.
- Check that transcript with the vendored DAG primitive before verification returns success.
- Cover multiple direct parents and linear chains through the same DAG validation path when the facts are representable.

**Non-Goals:**
- Do not verify compact splitting, base64url, JSON, Ed25519, BLAKE3 proof-reference derivation, or backend truth/freshness.
- Do not introduce async/network/default resolver behavior.
- Do not evaluate arbitrary caveat policy semantics.
- Do not claim failed backend calls produce complete transcripts; errors remain fail-closed ordinary Rust results.

## Decisions

### 1. Transcript as shell facts, verified as admission

**Choice:** Reuse the existing bridge entry facts from successfully parsed, signature-checked, revocation-clear tokens.
**Rationale:** These are the exact facts the existing DAG primitive is designed to validate.
**Alternative:** Re-prove live resolver/crypto/codec behavior. Rejected because those are explicit trust boundaries.

### 2. Parent-index DAG over returned proof tree

**Choice:** Compose parent transcripts before the child entry and add parent indices for the child.
**Rationale:** This matches topological ordering and keeps traversal facts deterministic.
**Alternative:** Keep only linear transcripts. Rejected because multi-proof tokens remain uninstrumented.

## Risks / Trade-offs

- If the existing primitive only admits a single root, the verified primitive must be narrowed/extended to admit a forest of independent roots that converge at the leaf. The boundary remains finite and structural.
