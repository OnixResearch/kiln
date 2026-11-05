# Lattice workflow contract

`lattice-workflow-contract` is the producer-owned interchange boundary for workflow consumers such as Kiln.

The crate is `no_std + alloc`. It contains only bounded values, deterministic BLAKE3 identity framing, and pure admission functions.

## Flow

1. Validate a `CapabilityProfile` and record `profile_identity`.
2. Build an ordered `WorkflowGraph` with immutable operation identities.
3. Set `graph.identity` with `graph_identity`.
4. Build a `WorkflowRequest` with exact consumer linkage.
5. Set `request.identity` with `request_identity`.
6. Call `validate_request` before transport.
7. Preserve the returned `WorkflowAccepted` identities.
8. Call `validate_observation` before a consumer changes domain state.
9. Bind controls and log reads to the exact accepted execution.

Equal admitted values produce equal identities. JSON object ordering does not affect identity framing.

## Fixtures

Generate the reviewed producer fixture:

```bash
cargo run --manifest-path crates/lattice-workflow-contract/Cargo.toml \
  --example emit-fixtures \
  > crates/lattice-workflow-contract/fixtures/producer-fixtures-v1.json
```

The integration test decodes this file independently. It checks accepted and rejected cycle, false-success, and stale-execution cases.

Nickel owns the human-reviewed structural contract at `contracts/kiln-workflow-contract.ncl`.

## Boundaries

The contract contains no command, credential, environment, path, provider payload, clock, transport, persistence, or execution authority.

Admission does not prove workflow correctness, host confinement, external effect success, credential freshness, or release readiness.

## License

New owned contract and transport source, including their schemas and fixtures,
uses AGPL-3.0-or-later; each package carries the full license in `LICENSE`.
Copyright (c) 2026 OnixResearch (contract) and Onix Research (transport).
`LICENSE-MIT-HISTORICAL` retains the original attributed MIT grants only for
previously licensed content. Those grants cannot be revoked, including for
byte-identical content still present. They do not grant MIT for new contributions.
This preparation does not refresh Kiln's immutable imported MIT snapshot.
