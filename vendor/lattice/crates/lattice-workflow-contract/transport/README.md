# Lattice workflow transport

`lattice-workflow-transport` provides a bounded Linux Unix-domain-socket exchange for `lattice-workflow-contract` values.

The crate contains two parts:

- a functional core for envelope identity, admission, linkage, and failure classification
- an imperative Unix shell for finite framed I/O, peer checks, deadlines, clients, and servers

## Handler boundary

Applications implement `WorkflowExchangeHandler`. The transport invokes the handler only after request identity and contract admission pass.

The handler owns workflow runtime behavior. The transport does not select operation catalogs, read credentials, execute processes, or grant authority.

## Server boundary

`serve` binds one explicit socket path, applies owner-only permissions, and handles the profile's finite connection count.

`serve_listener` accepts an already bound listener. It supports supervisors and local conformance tests that own listener creation.

Both paths require an ordered explicit Unix user-ID allowlist. The server checks peer credentials before it reads a request body.

## Client ambiguity

`UnixClient` returns these distinct classes:

- `ConnectFailed`: no request connection was established
- `NotAccepted`: the client did not finish writing the request frame
- `UnknownAfterWrite`: the request frame was complete, but no complete valid admitted response arrived

The client does not retry automatically.

After an unknown dispatch, `RecoverDispatch` uses the original admitted request to look up the exact acceptance. Observation-only recovery fails closed.

## Verification

From the Lattice repository root:

```text
nix develop -c cargo test --manifest-path crates/lattice-workflow-contract/Cargo.toml --package lattice-workflow-transport --all-targets --all-features
nix develop -c cargo clippy --manifest-path crates/lattice-workflow-contract/Cargo.toml --package lattice-workflow-transport --all-targets --all-features -- -D warnings
nix run .#workflow-transport-octet-deny-all
nix flake check -L --option secret-key-files ''
```

## Non-claims

Passing checks does not prove peer intent, workflow correctness, operation authority, runtime availability, external effect success, or production deployment.
