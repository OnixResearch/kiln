## Why

UCAN already exposes a verified multi-parent proof DAG transcript bridge for caller/shell-supplied finite facts, but the live verifier only records checked transcripts for no-proof and single-parent linear chains. Multi-proof tokens still execute ordinary Rust orchestration without a shell-emitted transcript routed through the DAG bridge.

## What Changes

- Emit a live proof-DAG transcript when recursive verification resolves a representable finite multi-parent proof forest/DAG.
- Route emitted DAG transcripts through `proof_traversal_dag_transcript_is_valid` before returning a verified token.
- Preserve existing caller-injected resolver, proof-store, revocation, signature, compact codec, and caveat-policy boundaries.

## Capabilities

### Modified Capabilities
- `library`: Documents and validates live multi-parent proof traversal transcript emission for already-resolved runtime proof facts.

## Impact

- **Files**: `src/token.rs`, docs, bridge guard/spec text, vendored verified-logic if the DAG primitive needs forest-root admission.
- **APIs**: No required public API expansion; this is live shell instrumentation over existing bridge APIs.
- **Testing**: Focused multi-parent proof verification tests, bridge guard, OpenSpec validation, full Cargo/Nix gates.
