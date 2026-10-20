## Context

Parsed caveat entries currently route required field-presence facts through `VerifiedParsedCaveatListShape`. The payload value is then base64url-decoded in the shell and rejected by `ucan-core` if the decoded payload is empty. A narrow verified seam can admit only finite shell-supplied payload facts without proving serde, base64url decoding, or caveat semantics.

## Goals / Non-Goals

**Goals:**
- Verify payload field presence, encoded non-empty, encoded base64url-no-pad, and decoded non-empty facts for every parsed caveat payload.
- Fail closed before core caveat construction if the verified predicate rejects the shell facts.
- Keep opaque caveat policy semantics caller supplied.

**Non-Goals:**
- Proving JSON parser correctness, UTF-8 extraction, duplicate-key behavior, base64url decoder correctness, decoded byte provenance, payload schema semantics, typed payload parsing, policy truth, clock/counter freshness, or token provenance.

## Decisions

### 1. Separate payload admission from caveat semantics

**Choice:** Add a dedicated vendored primitive and UCAN bridge for parsed caveat payload admission.

**Rationale:** The property is deterministic/data-only and distinct from existing required-field presence and typed caveat policy checks.

**Alternative:** Rely on `ucan-core` `CaveatView::new` to reject empty payloads. Rejected because it leaves a closeable parser/codec-adjacent shell fact outside the verified bridge inventory.

## Risks / Trade-offs

**Overclaiming codec correctness** → The docs and spec explicitly limit the claim to shell-supplied facts and keep base64url/serde correctness outside the bridge.
