# Verification evidence

## Baseline

The focused baseline passed before the core changes.

```console
nix develop path:$PWD -c cargo test -p kiln-provider-api -p kiln-core -p kiln-adapter-jj -p kiln-adapter-cob -p kiln-adapter-remote -p kiln-adapter-radicle
```

## Implementation evidence

`kiln-provider-api` owns the migrated source, collaboration, remote, and trigger contracts. The four adapter crates no longer define competing public port traits.

Pure core functions decide Jujutsu ambiguity, COB freshness, remote capability admission, trigger identity, and infrastructure-failure disposition.

Positive and negative tests cover effect order, stale observations, divergence, unsupported operations, disconnection, timeout, and provider substitution.

## Gate evidence

These commands passed:

```console
nix develop path:$PWD -c cargo check -p kiln-core --target wasm32-unknown-unknown
nix develop path:$PWD -c cargo test --workspace
nix develop path:$PWD -c cargo clippy --workspace --all-targets -- -D warnings
nix run path:/home/brittonr/git/OnixResearch/cairn#cairn -- validate --root $PWD
nix run path:/home/brittonr/git/OnixResearch/cairn#cairn -- gate proposal align-provider-port-ownership --root $PWD
nix run path:/home/brittonr/git/OnixResearch/cairn#cairn -- gate design align-provider-port-ownership --root $PWD
nix run path:/home/brittonr/git/OnixResearch/cairn#cairn -- gate tasks align-provider-port-ownership --root $PWD
nix flake check path:$PWD -L
```

The pre-sync Tracey command reported only the five requirement IDs in this delta as dangling. Specification sync made these IDs accepted requirements.

The final Tracey gate passed with `361/361` referenced requirements.
