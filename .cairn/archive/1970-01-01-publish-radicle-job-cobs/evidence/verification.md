# Verification evidence

## Result

Radicle job publication writes bounded local `xyz.radworks.job` objects through
one explicit SecretSpec signer declaration. Writes enforce exact RID, object
type, signer, object identity, head, payload, scan, run and idempotency checks.
Unknown writes require read-only reconciliation.

## Evidence

- `cargo test -p kiln-adapter-radicle --features status-native-fixtures`: 83
  passed, 0 failed, including job unit and native coverage.
- `cargo test -p kiln-web-service --test cobs`: 11 passed, 0 failed.
- Full workspace tests: 1213 passed, 0 failed, 1 ignored.
- Workspace and Radicle-adapter Clippy pass with `-D warnings`; rustfmt and
  `cargo check --locked --workspace --all-targets` pass.
- Cairn validation passes with zero issues. Traceability coverage reports zero
  missing requirements; active-change IDs remain dangling until acceptance.

## Non-claims

Local job writes do not prove peer convergence, remote visibility or node
availability. A job object does not complete a Kiln attempt or change CI state.
The browser fixture does not verify signatures, actor authority or reduce job
state. The profile grants no merge, identity, delegation, cache, network or
release authority.
