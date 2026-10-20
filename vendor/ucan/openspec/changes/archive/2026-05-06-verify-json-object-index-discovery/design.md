## Context

`src/token.rs` currently computes decoded JSON significant-byte indexes with ordinary Rust:

- find first non-RFC-8259-whitespace byte;
- find last non-RFC-8259-whitespace byte;
- pass both indexes plus bytes into `VerifiedJsonObjectBytes` / `json_object_bytes_are_valid`.

`verified_logic::ucan_json` already contains verified whitespace and object-envelope predicates over shell-supplied indexes. The gap is that the live shell still owns the discovery of those indexes.

## Goals / Non-Goals

**Goals:**

- Verify deterministic discovery of first/last significant decoded JSON bytes for finite byte buffers.
- Route live decoded header/payload object admission through the new vendored helper.
- Preserve the exact accepted/rejected envelope behavior currently protected by tests.
- Document the narrower trust boundary after the change.

**Non-Goals:**

- No JSON parser verification, UTF-8/string extraction verification, serde field correctness, canonicalization, duplicate-key semantics, base64url decoder proof, cryptographic proof, backend/proof truth, clock proof, or token provenance proof.
- No dependency on ambient state, filesystem/network lookup, process-global configuration, or hidden fallback parsing.
- No change to accepted compact-token semantics beyond replacing the local index-discovery trust seam with a verified predicate.

## Decisions

### 1. Verified-logic owns index discovery

**Choice:** Add the executable index-discovery/object-envelope helper in `../verified-logic` rather than only wrapping the existing UCAN-local scan.

**Rationale:** The property is deterministic, finite, byte-level, and independent of UCAN std-shell I/O/runtime dependencies. It fits the verified-logic crate better than UCAN.

**Alternative:** Keep UCAN-local index discovery and add more tests. Rejected because tests would not reduce the shell-supplied fact boundary.

### 2. Preserve explicit decoded-byte boundary

**Choice:** The new helper consumes decoded bytes only and does not move base64url decoding into the verified claim.

**Rationale:** The current verified-logic surface has structural base64url segment admission, not a verified decoder. Keeping this split prevents overclaiming.

### 3. Fail closed before serde

**Choice:** UCAN must call the new bridge before `serde_json::from_slice`, and failures must remain JSON decode errors with no fallback parser path.

**Rationale:** The existing parse pipeline already guards serde with structural object admission. The change should reduce trust while preserving the fail-closed ordering.

## Risks / Trade-offs

**Verus proof complexity for reverse scans** → Keep the primitive small: either prove first/last scans directly with prefix/suffix invariants or expose a helper that validates computed indexes while `verified-logic` also computes them.

**Behavior drift** → Add positive and negative tests around leading/trailing JSON whitespace, empty input, whitespace-only input, arrays, scalar bytes, and nested-object bytes whose envelope is top-level object-like.

**Archive/doc drift** → Update `docs/verified-bridge-coverage.md` and the canonical library spec during implementation, and rerun `scripts/check-ucan-bridge.sh`.
