# Nix build adapter

`kiln-adapter-nix` is Kiln's direct Nix derivation build provider. It executes
one exact `.drv` with the pinned Nix CLI, admits the native JSON result, and
returns the canonical Kiln build observation. It is distinct from
`kiln-adapter-nix-eval`, which only discovers jobs through `nix-eval-jobs`.

## Pinned cohort

The profile binds:

- Nix version `2.36.0`;
- executable path `/nix/store/6rwk5j1qqk7na4la5m2ka34p734braxa-nix-2.36.0/bin/nix`
  and BLAKE3 identity
  `0e670a46c789a33272d9193080b91affe8023af27bcfa9fad76d3d959178a8e7`;
- request schema `kiln.nix-build-request.v1`;
- observation schema `kiln.nix-build-observation.v1`;
- receipt schema `kiln.nix-build-receipt.v1`;
- native report schema `nix.build-json.v1`;
- platform `x86_64-linux`;
- Bounded Exec revision `29dac88ecded94457572db3fdfaaaab95fa91525`.

The profile also binds one safe absolute store path, one derivation directly
under that store, the derivation BLAKE3 identity, the exact plan and policy
identities, the required output names, a worker identity, a store identity, and
a writable home directory. Admission fails before any process starts if the
executable, derivation, cohort fields, capabilities, limits, or non-claims
differ from the pinned values.

## Dispatch

The pure request lowering requires the build plan's provider profile, plan
identity, policy identity, platform, and sorted products to equal the admitted
profile. The invocation is:

```console
nix --extra-experimental-features nix-command build \
  --no-link --json --offline --expr 'import /nix/store/…-name.drv'
```

Bounded Exec runs it with a cleared environment, a fixed `HOME`, null stdin, a
timeout, bounded stdout/stderr, and process-group teardown. `NIX_CONFIG`
disables flake configuration, the flake registry, substituters, and remote
builders. The derivation and all of its inputs therefore must already be
available in the local store. This is intentional: the adapter does not claim
network acquisition, substitution, remote execution, or flake evaluation.

The pinned Nix JSON form is:

```json
[{"drvPath":"/nix/store/…-name.drv","outputs":{"out":"/nix/store/…-name"}}]
```

## Completion

A zero exit is a successful build only when the native report contains exactly
one result, its `drvPath` equals the admitted derivation, and every required
product appears exactly once as a direct child of the admitted store. Each
product binds a Kiln BLAKE3 artifact identity over the output name and path and
a separate store-object identity over the path. The observation also records
the executable identity as the builder identity, the profile worker and store
identities, a native receipt, and one bounded stderr log identity.

Outcomes are terminal and explicit:

| Native condition | Kiln outcome |
|---|---|
| Zero exit with every required output | `Success` |
| Nonzero exit | `Failed` |
| Timeout, cancellation | `TimedOut`, `Cancelled` |
| Signalled, output limit, malformed report, missing output, wrong derivation, or output outside the store | `ProviderError` |
| Process start that may have occurred without an observation | `Unknown` |

Every non-unknown outcome has a bounded receipt identity. A repeated dispatch
returns the recorded observation without running a second process. A changed
plan or request for the same effect fails closed. `observe` and `reconcile`
return only the exact recorded observation. `restore` admits an existing
observation without claiming that its old log bytes are available; a log read
after an observation-only restore fails explicitly.

## Verification

The fixture-only rail does not need a Nix daemon:

```console
cargo test -p kiln-adapter-nix --offline
```

The real local rail builds a trivial derivation with the pinned executable:

```console
cargo test -p kiln-adapter-nix --features nix-live --offline -- --nocapture
```

The Nix check `nix-build-adapter-fixtures` runs the fixture-only tests inside
the Nix sandbox. A real build is deliberately outside that sandbox because the
sandbox cannot write a nested Nix store.

## Non-claims

Passing these checks proves only that this adapter, exact executable, exact
`.drv`, local store inputs, and native JSON result were admitted. It does not
prove build correctness, sandbox completeness, derivation provenance or source
trust, output content correctness, cache truth, reproducibility, or release
readiness.
