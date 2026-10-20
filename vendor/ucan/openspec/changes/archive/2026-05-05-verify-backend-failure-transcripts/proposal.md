# Change: Verify backend failure transcript boundary

## Why
The UCAN shell already fails closed when caller-injected key resolver, proof store, or revocation backends fail, but that behavior is encoded as ordinary Rust control flow. A small verified transcript boundary can make the fail-closed decision explicit over finite caller/shell-supplied backend operation facts without claiming backend truth, freshness, or I/O correctness.

## What Changes
- Add a verified finite backend-operation transcript primitive for key resolution, proof loading, and revocation checks.
- Bridge the primitive through UCAN public verified helpers.
- Emit/check transcript facts at live backend call sites and fail closed if an operation did not report success.
- Add tests and docs covering key resolver, proof store, and revocation backend failures as denied verification outcomes.

## Non-Goals
- Verifying resolver/proof-store/revocation backend correctness, freshness, availability, provenance, or persistence.
- Verifying compact token parsing, cryptography, proof-reference hashing, network/filesystem behavior, or ambient runtime behavior.
- Changing resolver/proof-store/revocation traits or adding async/runtime dependencies.
