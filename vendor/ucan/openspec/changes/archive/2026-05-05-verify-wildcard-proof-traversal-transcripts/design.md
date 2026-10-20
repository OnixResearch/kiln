## Context

`verified_logic::ucan_proof_traversal` currently treats traversal capability containment as resource-prefix plus exact ability byte equality. UCAN's live shell uses `VerifiedCapability` and accepts slash-separated wildcard ability segments, with `*` valid only as a full segment. As a result, the live verifier deliberately omits transcripts for wildcard linear chains.

## Goals

- Reuse the existing verified UCAN ability module for wildcard ability containment inside proof traversal transcripts.
- Preserve the current transcript data-only boundary: callers still supply finite entries, visited references, revocation facts, and authorization targets.
- Keep UCAN resolver/proof-store/revocation traits synchronous and caller-injected.
- Allow live exact and wildcard single-parent proof chains to emit and check transcripts.

## Non-Goals

- No multi-parent DAG transcript/folding primitive.
- No compact-token codec, signature, proof-reference hash, revocation backend, caveat-policy, persistence, replay, network, filesystem, or ambient-clock verification.
- No new public resolver or backend behavior.

## Decisions

### Ability representation

**Choice:** Convert traversal capabilities to verified-logic `Ability` segment vectors before invoking proof traversal semantics.

**Rationale:** The sibling verified-logic crate already has machine-checked UCAN ability wildcard containment over segment vectors. Reusing it avoids a second, inconsistent wildcard implementation.

### Live shell eligibility

**Choice:** Keep live transcript eligibility to no-proof and single-parent proof chains, but allow both exact and wildcard ability attenuation when the bridge accepts the transcript.

**Rationale:** This closes the current explicit wildcard gap without claiming multi-parent DAG folding is verified.

## Risks / Trade-offs

- Changing verified-logic traversal capability representation requires UCAN bridge conversion updates. Mitigation: keep UCAN public bridge structs unchanged and perform conversion internally.
- Multi-parent chains remain shell-local. Mitigation: document that as the next explicit proof-traversal seam.
