# Verification evidence

## Result

The Radicle CI project profile admits non-empty branch and tag source
references. Branch sources project `Push`; tag sources project the new `Tag`
event. In both cases the alias is the short branch or tag name and the exact
revision remains the trigger's `after`.

## Evidence

- `cargo check --workspace --all-targets` passed.
- Radicle CI tests pass, including branch/tag event and alias projection.
- Full workspace tests and workspace-wide Clippy pass.
- `cargo fmt` and Cairn validation pass.

## Non-claims

The broker does not yet emit tag events. This profile supports exact local or
future broker tag intake; it does not add trust, approval or release authority.
Push behavior is unchanged.
