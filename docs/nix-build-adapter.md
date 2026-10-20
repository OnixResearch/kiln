# Nix build adapter

`kiln-adapter-nix` is Kiln's direct Nix derivation build provider. It executes
one exact `.drv` with the pinned Nix store CLI, admits the native realisation
roots, and returns the canonical Kiln build observation. It is distinct from
`kiln-adapter-nix-eval`, which only discovers jobs through `nix-eval-jobs`.
`kiln-nix-build` is the composition and operator shell that maps a discovery
`TaskProposal` to this provider.

## Pinned cohort

The profile binds:

- nixpkgs revision `346dd96ad74dc4457a9db9de4f4f57dab2e5731d`;
- Nix version `2.34.8`;
- executable paths
  `/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix` and
  `/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix-store`,
  both with BLAKE3 identity
  `82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852`;
- request schema `kiln.nix-build-request.v1`;
- observation schema `kiln.nix-build-observation.v1`;
- receipt schema `kiln.nix-build-receipt.v1`;
- native report schema `nix-store-realisations.v1`;
- platform `x86_64-linux`;
- Bounded Exec revision `29dac88ecded94457572db3fdfaaaab95fa91525`.

The profile also binds one safe absolute store path, one derivation directly
under that store, the derivation BLAKE3 identity, the exact plan and policy
identities, the exact required output names and paths, a private GC-root base,
a worker identity, a store identity, and a writable home directory. Admission
fails before any process starts if the executable, derivation, cohort fields,
capabilities, limits, or non-claims differ from the pinned values.

## Dispatch

The pure request lowering requires the build plan's provider profile, plan
identity, policy identity, platform, and sorted products to equal the admitted
profile. The invocation is:

```console
nix-store --add-root /var/lib/kiln/nix-build-roots/…/root --indirect \
  --realise /nix/store/…-name.drv
```

Bounded Exec runs it with a cleared environment, a fixed `HOME`, null stdin, a
timeout, bounded stdout/stderr, and process-group teardown. `NIX_CONFIG`
disables flake configuration, the flake registry, substituters, and remote
builders. The derivation and all of its inputs therefore must already be
available in the local store. The command creates a GC root for every realised
output under the effect-specific private root directory, so a successful
product cannot be collected before the shell releases it. This is intentional:
the adapter does not claim network acquisition, substitution, remote execution,
or flake evaluation.

The native output is one root path per output, for example:

```text
/var/lib/kiln/nix-build-roots/<effect>/root
/var/lib/kiln/nix-build-roots/<effect>/root-dev
```

The adapter reads each root target and requires it to equal the exact expected
store path bound in the profile. It does not infer output names from root
suffixes.

## Completion

A zero exit is a successful build only when the native realisation contains
exactly one root per expected output, every root is inside the effect-specific
GC-root directory, and every root target equals the exact expected store path
bound in the profile. Each product binds a Kiln BLAKE3 artifact identity over
the output name and path and a separate store-object identity over the path. The observation also records
the executable identity as the builder identity, the profile worker and store
identities, a native receipt, and one bounded stderr log identity.

Outcomes are terminal and explicit:

| Native condition | Kiln outcome |
|---|---|
| Zero exit with every required output | `Success` |
| Nonzero exit | `Failed` |
| Timeout, cancellation | `TimedOut`, `Cancelled` |
| Signalled, output limit, malformed roots, missing output, duplicate output, or output path mismatch | `ProviderError` |
| Process start that may have occurred without an observation | `Unknown` |

Every non-unknown outcome has a bounded receipt identity. A repeated dispatch
returns the recorded observation without running a second process. A changed
plan or request for the same effect fails closed. `observe` and `reconcile`
return only the exact recorded observation. `restore` admits an existing
observation without claiming that its old log bytes are available; a log read
after an observation-only restore fails explicitly. `release_output_roots`
removes the effect-specific GC-root directory only after downstream durable
handoff; it is idempotent.

## Composition and operator surface

`kiln-nix-build` maps a discovery `TaskProposal` to an exact profile and plan:

- it requires the task build provider to match the admitted profile;
- it requires the derivation to be one direct `.drv` child of the store;
- it binds the task output names and expected store paths exactly;
- it hashes the `.drv` before construction; and
- it derives the plan's sorted products from the task outputs.

The `kiln-nix-build` binary accepts a Nickel-exported profile and JSON documents:

```console
nickel export config/nix-build-profile.ncl > base.json
kiln-nix-build from-task --profile base.json --task task.json \
  --intent intent.json --log build.log --release
```

`from-task` is the discovery-to-build composition and accepts the Nickel base
profile because the task supplies the exact derivation and output paths.
`build` and `doctor` require a fully bound profile whose derivation identity
and expected outputs are already concrete. `--release` removes the output
GC roots after the observation is printed; omit it when a downstream durable
handoff still needs the products.

## Verification

The fixture-only rail does not need a Nix daemon:

```console
cargo test -p kiln-adapter-nix --offline
cargo test -p kiln-nix-build --offline
```

The real local rail realizes a trivial derivation with the pinned executable
and checks its rooted output:

```console
tests/nix-build-live.sh
```

The host rail runs both `--features nix-live` suites; it requires a writable
local Nix store and the pinned executable.

The Nix checks `nix-build-adapter-fixtures` and `nix-build-adapter-cohort` run
the fixture-only tests and verify the repo-pinned Nix executable inside the Nix
sandbox. A real build is deliberately outside that sandbox because the sandbox
cannot write a nested Nix store; `tests/nix-build-live.sh` is the host rail.

## Non-claims

Passing these checks proves only that this adapter, exact executable, exact
`.drv`, exact expected output paths, local store inputs, GC-root retention, and
native realisation roots were admitted. It does not prove build correctness,
sandbox completeness, derivation provenance or source trust, output content
correctness, cache truth, reproducibility, or release readiness.
