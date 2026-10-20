## Context

`VerifiedCapability::contains` covers resource and ability attenuation. The shell also requires child capabilities to preserve parent caveats, but the current nested comparison is ordinary Rust. Vendored `ucan_token::check_attenuation_chain` does not model the shell's opaque `CapabilityDocument` caveat preservation paired with the same parent capability.

## Goals / Non-Goals

**Goals:** precisely specify the invariant and require evidence before changing the shell check.

**Non-Goals:** no automatic interpretation of caveat payloads, no weakening of `DenyUnknownCaveats`, no replacement of caller-supplied caveat policy, and no new persistence/network/time behavior.

## Decisions

### 1. Same-parent pairing is mandatory

**Choice:** A child capability must be accepted only when one parent capability both contains the child resource/ability and has all of its caveats preserved by the child.

**Rationale:** Splitting containment and caveat preservation across different parent capabilities would accidentally broaden delegation.

**Alternative:** Use any parent for containment and any parent for caveat preservation. Rejected as unsound for opaque caveat attenuation.

### 2. Do not force a verified bridge if no exact primitive exists

**Choice:** Implementation may first add focused shell regressions and documentation if upstream verified-logic lacks an exact caveat-preserving attenuation primitive.

**Rationale:** Routing through a non-equivalent helper would create false confidence.

## Risks / Trade-offs

**False equivalence with existing attenuation helper** → The implementation MUST prove or document equivalence before replacing shell-local logic.
