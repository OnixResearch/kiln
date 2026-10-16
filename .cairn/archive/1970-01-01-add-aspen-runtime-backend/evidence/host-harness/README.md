# Cross-repository Aspen host harness

This harness is evidence-only. It does not add a sibling worktree dependency to Kiln.

Required revisions:

- Aspen: `171f25b5a9e878390af8f20b9797f6096cca5097`
- Bounded Exec: `29dac88ecded94457572db3fdfaaaab95fa91525`
- Kiln: the working revision under review

Set `ASPEN_ROOT` and `KILN_ROOT` to clean worktrees at those revisions. Then render the two `.in` files into a temporary Cargo project.

Replace `__ASPEN_ROOT__` and `__KILN_ROOT__` with the absolute worktree paths. Build `kiln-aspen-extension` from `KILN_ROOT`.

Run the harness from Aspen's development shell. Set the unusual Cargo binary variable with `env`, because its name contains hyphens:

```console
env 'CARGO_BIN_EXE_molten-native-extension-fixture=/path/to/kiln-aspen-extension' \
  CARGO_TARGET_DIR=/path/to/cargo-target \
  nix develop "path:$ASPEN_ROOT" -c \
  cargo run --quiet --manifest-path /path/to/rendered/Cargo.toml
```

The expected output is `../aspen-host-receipt.json`. The fixture uses Aspen's real `NativeSystemExtensionService` and bounded process executor. It does not use `LocalAspenService`.
