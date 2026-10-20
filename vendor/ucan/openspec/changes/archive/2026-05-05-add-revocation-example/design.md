## Context

The revocation checker API is caller-injected and shell-only. The example should reinforce that boundary and keep compact UCAN token strings out of logs.

## Decisions

### Use deterministic local keys and `RevocationList`

**Choice:** Mirror the proof-chain example's deterministic local issuers, but add a `RevocationList` containing the parent `ProofReference`.

**Rationale:** This demonstrates the supported API without inventing registry, credential-status, database, network, or replay semantics.

### Redact token material by omission

**Choice:** Print issuer/proof/reference metadata only, not compact tokens.

**Rationale:** Compact tokens are bearer-like values and should not appear in example output.

## Risks / Trade-offs

- The example is intentionally local/in-memory; production registry design remains application-owned.
- The example exercises parent revocation. Leaf revocation remains covered by unit tests and documented API behavior.
