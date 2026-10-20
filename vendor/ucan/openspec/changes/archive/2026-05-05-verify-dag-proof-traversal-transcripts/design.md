## Context

The shell already emits/checks verified transcripts for no-proof tokens and exact/wildcard single-parent chains. Proof DAG recursion still combines multiple parent delegations in ordinary Rust.

## Goals

- Model finite multi-parent proof DAG transcripts as data only.
- Require topological parent indices so cycles are rejected structurally.
- Require every child capability to be authorized by at least one parent and every listed parent caveat prefix to be preserved.
- Preserve the current trust boundary: no proof fetching, parsing, signature crypto, backend correctness, clocks, filesystem, or network behavior is verified by this primitive.

## Non-Goals

- No async/runtime/network proof resolution.
- No hidden default proof store or revocation registry.
- No claim that compact token parsing, signatures, resolver freshness, or caveat policy semantics are formally verified.

## Decisions

### Finite transcript API

**Choice:** Add a separate DAG transcript bridge rather than changing the linear transcript shape.

**Rationale:** Linear transcripts remain simple and backwards-compatible while DAG callers can provide parent indices explicitly.

### Topological ordering

**Choice:** Parent indices must be less than their child index.

**Rationale:** This makes acyclicity finite and cheap to verify without recursive graph search.
