## Context

The shell verification workflow remains richer than the vendored structural chain checker: it performs token parsing, signature verification, resolver/proof-store traversal, revocation checks, wildcard-aware delegation containment, caveat preservation, caveat policy evaluation, and structured errors. The vendored chain checker composes finite resolved-entry fields: issuer/audience linkage, caller-provided signature-valid booleans, exact attenuation narrowing, and temporal validity.

## Decisions

### 1. Add an explicit bridge input type

**Choice:** Introduce `VerifiedChainEntry` with public issuer/audience strings, validated capability documents, time bounds, and a caller-provided `signature_valid` flag.

**Rationale:** The verified primitive expects a finite vector of `ChainEntry` values. A shell-owned input type keeps vendored internals out of the public API and allows validation before conversion.

### 2. Keep the bridge structural-only

**Choice:** Expose the helper as an explicit structural check rather than wiring it into the main verifier.

**Rationale:** The main verifier supports wildcard ability containment and caveat preservation, while the vendored structural checker uses exact ability equality and has no caveat-policy model. Using it as a replacement would reject legitimate shell chains or weaken policy boundaries. The bridge is useful for exact structural cross-checks and future integration slices where the semantics match exactly.

## Non-Goals

- No proof resolution, network/filesystem fallback, revocation registry, signature cryptography, key resolution, caveat policy, wildcard containment replacement, persistence, or ambient clock behavior.
- No change to the existing token verification decision path.
