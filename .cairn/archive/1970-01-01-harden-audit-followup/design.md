## Context

The earlier audit mixed public identity comparisons and a Rust model with production authentication. The deployed Worker in src/index.mjs validates timestamps before object resolution and atomically persists append and control mutation nonces. Root Cargo dependencies target Rust 1.88; standalone runtimes and vendored producers have independent, immutable cohorts.

## Decisions

### Decision: Harden the deployed authentication primitive

**Choice:** Preserve the signed framing, timestamp window, exact-operation reconciliation, and SQLite replay transaction; use WebCrypto HMAC verification instead of a hand-written comparison. Remove the unbounded model-only nonce set. Verify stale/future signatures, durable append replay, and valid fresh operation retry with executable Worker fixtures.

**Rationale:** The platform primitive owns constant-time MAC verification. A process-local cache neither survives restart nor protects the actual endpoint, and adding an unlimited set violates boundedness. Read-only requests remain timestamp-limited; this change does not introduce a new durable consume-once read protocol.

### Decision: Preserve public identity semantics and tighten the socket boundary

**Choice:** Restore ordinary comparisons for public BLAKE3 identities and local all-zero seed admission; keep Aspen socket mode 0600 and reject a symlinked, non-directory, group/other-accessible, or different-owner final parent before binding. Exercise the real bound socket. Public operator allowlist and request equality stay unchanged.

**Rationale:** Public equality timing is not secret disclosure. An OR reduction is not a cryptographic constant-time guarantee. A private owned final directory closes the pre-chmod connection and path-replacement opportunity for other users; stable ancestor provisioning remains operator policy, not a sandboxing claim. Owner-only socket access intentionally requires same-user host/client deployment.

### Decision: Update compatible registry dependencies only

**Choice:** Use Cargo-generated updates targeted at advisory fixes that satisfy current API, provider pins, and Rust 1.88. Preserve imported producer snapshots and standalone runtime locks; record unresolved advisories with exact constraints instead of suppressions or unreviewed forks.

**Rationale:** Lockfile maintenance can remove reachable known defects without silently raising the toolchain or changing independently qualified source cohorts. An advisory identifies an affected package, not application reachability.

## Risks / Trade-offs

- Owner-only socket access intentionally denies previously implicit group access; deployment documentation must state the same-user requirement.
- Some advisory fixes require newer provider APIs, Rust versions, or immutable producer imports and cannot be safely forced into this change.
- Passing local conformance does not qualify the changed executable cohort for production or publication.
- Existing clock behavior uses the shell's observed time and preserves the reviewed five-minute window; no global clock or revocation guarantee is introduced.
