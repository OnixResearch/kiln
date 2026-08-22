# Design: Iroh observation shell

## Context

Operators may observe Kiln from peers without a public endpoint. The accepted observation core projects views from committed state; the HTTP shell serves those views to standard clients. This change serves the same views to iroh peers over one ALPN protocol.

The n0-computer survey established the pinned primitives: `iroh` provides QUIC dial-by-key, hole-punching, and relay fallback; `iroh-blobs` and `bao-tree` provide BLAKE3-verified streaming. These align with Kiln's BLAKE3 identity discipline and with the accepted `kiln.observe.view.v1` identity contract.

## Goal and Success Contract

The goal is a narrow Kiln-owned iroh shell that serves the accepted projection to admitted peers with verified payload streaming and provable equality with the HTTP leg.

Completion requires a pinned n0 cohort, one ALPN protocol, peer identity and trust policy, verified streaming, an equivalence rail, full positive and negative conformance, and documented non-claims.

False completion includes a claim of delivery without identity verification, an unadmitted peer serving, divergent legs, or a leak of relay or transport facts into canonical state.

## Decisions

### Decision: Own a narrow iroh adapter

`kiln-adapter-iroh-observe` is Kiln-owned and narrow. It serves only the accepted observation projection. It reuses n0 patterns and the in-stack identity discipline of the tile remote-peer model, but it does not consume tile's terminal-workspace transport contract and does not copy tile or aspen source.

Rejected alternatives: consuming tile's `WorkspaceTransport` contract (terminal-workspace semantics Kiln does not own); copying tile transport source (violates the stack reuse rule); building a custom QUIC layer (iroh already provides it).

### Decision: Pin the n0 cohort

The profile pins `iroh`, `iroh-blobs`, and `bao-tree` with exact versions and revisions and records the ALPN identity `kiln.observe.v1`. Cohort drift fails before serving.

### Decision: Dial by key with explicit trust

Nodes hold a stable Ed25519 secret identity. Peers dial by public key under an alias allowlist with explicit revocation. Denied peers get a bounded denial. Relay fallback is allowed but visible as a labeled transport fact; it grants no authority.

### Decision: Verify before claiming delivery

Log and record ranges stream as content-addressed payloads through the `iroh-blobs` and `bao-tree` path. A recipient claims delivery only after the declared BLAKE3 identity verifies. Corruption is rejected or refetched and reported.

### Decision: Prove equivalence with the HTTP leg

A maintained rail asserts that HTTP and iroh legs produce equal logical views over shared fixtures. Divergence fails conformance. The wire payload contract is transport-independent; equality is the check of that contract.

### Decision: Keep the shell a tokio shell

The iroh adapter is an imperative tokio shell. The accepted projection core stays pure `no_std + alloc`. No async or network type enters the core.

### Decision: Keep loss and relay non-authoritative

Dropped, delayed, relayed, or failed payloads are transport observations. They never change canonical state, run outcomes, gates, or release readiness.

### Decision: No write authority

The iroh shell performs no command intake. Controls are a later change through the existing command and outbox path.

## Cross-stack evaluation and reuse

Sibling reuse follows the OnixResearch owner grant: public license conflicts do not block reuse of source the stack exclusively controls. Third-party notices and vendored material remain documented and preserved.

- `host-control-core` is adopted as the wire-contract framing substrate shared with the HTTP leg: snapshot-first streams, bounded observations, selectors, waits, and feature negotiation. CI content stays in Kiln-owned payloads.
- Observation authority uses the basalt boundary with ucan enforcement behind the observation-authn port. Peer principals align to `did:key` Ed25519 so iroh public keys and capability principals share one identity model.
- `molten` (aspen) is the stack's runtime and transport owner and is evaluated as an alternative transport home for this leg. Adoption is not assumed: its `iroh_sam_dataspace` still carries unestablished tracey debt, so the committed plan keeps the pinned n0 cohort behind the transport port, and molten may replace the transport only on passing evaluation evidence.
- `valence` (through the existing adapter) may record each served view as `recorded_only` evidence.

The n0 cohort pins remain independent of any sibling component.

## Configuration

Typed Nickel profiles declare the cohort pins, ALPN identity, endpoint and relay policy, peer identity and trust policy, named bounds, and non-claims. Positive and negative profile fixtures validate the contract.

## Validation

Positive fixtures cover serving to admitted peers, verified delivery, and HTTP/iroh equality. Negative fixtures cover cohort drift, unknown and revoked peers, corrupted payloads, transport failure, relay dependence, divergent legs, missing negative cases, and authority promotion.

## Claim Boundary

Passing checks establish bounded iroh serving of the accepted projection with verified payloads and HTTP/iroh equality over shared fixtures. They do not prove transport acceptance, live completeness, arbitrary-host support, provider correctness, CI success, or release readiness.
