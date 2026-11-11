# Bind Lattice implementation request dispatch

## Why

Issue #45 owns the handoff from a reserved implementation attempt to an explicitly
admitted Lattice route, without turning transport acceptance into implementation
completion.

## What Changes

- Add request admission binding the reservation plan, attempt/fence, repository,
  base, output and cancel-only controls.
- Require an agent/local/custom graph and validate the selected Lattice profile.
- Add a transport dispatch helper with explicit disconnect/unknown handling and
  acceptance-linkage validation.
- Keep workflow creation and worker choice external.

## Impact

- **Files**: `crates/kiln-adapter-lattice/src/implementation.rs`, docs and spec.
- **Testing**: exact request binding and mismatched-attempt refusal.
- **Authority**: bound transport dispatch only; no worker, workflow creation or completion claim.
