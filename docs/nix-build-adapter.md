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

Bounded Exec runs it with a cleared environment, a fixed `HOME` and
`NIX_CONF_DIR`, null stdin, a timeout, bounded stdout/stderr, and process-group
teardown. `NIX_CONFIG` disables substituters and remote builders, enables the
Nix sandbox, and leaves experimental features unset. The derivation and all of
its inputs therefore must already be available in the local store.

Before the rooted build, the shell runs a bounded
`nix-store --realise --dry-run <drv>` preflight with the same environment. A
failed, timed-out, cancelled, or oversized preflight returns before any rooted
effect. The command creates a GC root for every realised
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
the output name and path and a separate store-object identity over the path.
The adapter deliberately uses the content-addressed store path as the bounded
product identity. When the runtime can observe it, a separate store-object fact
records the NAR SHA-256/size and references; that fact does not replace the
product identity. The observation also records
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
and expected outputs are already concrete. Both dispatch paths run the bounded
preflight. `--release` removes the output GC roots after the observation is
printed; omit it when a downstream durable handoff still needs the products.

## Derivation admission

Before dispatch, the composition reads the exact `.drv` under the profile's
`max_derivation_bytes`, parses it with the Apache-2.0 `nix-derivation` crate, and
admits it against the exact platform and task output paths. Admission requires a
non-empty output set, matching output names, a matching `system`, a bounded
builder, and exact declared output paths where the derivation supplies them.
Input sources and input derivations are recorded. Input-addressed output paths
that need input-modulo hashes are accepted only as task-bound expectations; Kiln
does not claim it recomputed them. A mismatch fails before any preflight or
build process starts.

The admitted structural identity is recorded separately from the profile's
file-content identity. The existing profile binding still hashes the `.drv`
bytes; the admitted identity binds path, platform, outputs and inputs.

## Product and input facts

When the runtime can observe them, Kiln records per-product store facts:
lowercase hexadecimal NAR SHA-256, NAR size, sorted references, reference
completeness, deriver, content address and signatures. Native SHA-256/NAR
values never replace Kiln BLAKE3 identities. Reference scanning uses the exact
32-byte Nix-base32 hash parts of every provable input source, known input
derivation path and output path. `references_complete` is false when an input
derivation's outputs remain unresolved.

Input sources are also observed as exact NAR facts, sorted by path, and bound by
a BLAKE3 `input_closure_identity`; `input_closure_complete` is false when an
unresolved input derivation prevents a complete input closure. Products from the Mantle adapter record NAR
facts for exported paths; Mantle does not currently expose a complete input
closure, so those products carry `references_complete = false`.

The plan and observation also bind typed constraints: system, optional minimum
memory, sorted read-only paths, network access and shell provision. The
`kiln-nix-build` composition derives the system and shell policy from the
admitted derivation. These facts do not prove hermeticity, sandbox completeness
or build correctness.

## Read-only cache projection

`kiln-nix-cache render --document projection.json --output /absolute/dir`
writes a bounded static Nix binary-cache projection: `nix-cache-info`,
canonical `.narinfo` records and NAR bytes. The projection document is
`kiln.nix-cache-projection.v1`; it names a store directory and exact product
paths plus recorded NAR facts. Missing NAR files are encoded from the exact
paths, while supplied NAR files are copied and verified against the recorded
size and SHA-256. References must be complete. The command does not serve HTTP,
sign with operator keys, substitute, or mutate a store.

The Apache-2.0 `nix-derivation`, `nix-archive` and `nix-narinfo` crates provide
the format semantics. Snix's GPL-3.0 crates are not linked into Kiln; Mantle
retains its vendored Snix data layer under its own AGPL boundary.

### Durable state

Add `--state STATE.json` to `from-task` or `build` to persist an intent before
dispatch and the terminal observation after dispatch. On restart:

```console
kiln-nix-build observe --state state.json
kiln-nix-build reconcile --state state.json
kiln-nix-build release --state state.json
```

The state stores the exact profile source, plan, observation, and release
state. An existing observation is restored without rebuilding. A state without
an observation is an interrupted intent and is refused rather than silently
rebuilt. `release` is idempotent and updates the state.

A task-derived state rehydrates by hashing the task derivation. Keep the
discovery adapter's derivation GC roots until the terminal observation is
stored, then release them. The build adapter roots realized outputs, not the
derivation itself; the state does not add a derivation root.

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

The host rail first realizes the pinned `nix-eval-jobs` cohort and adds the
fixture flake as an immutable store source, then runs both `--features
nix-live` suites, including real discovery → build, timeout, cancellation, and
multi-output cases. It requires a writable local Nix store and the pinned
executables. It is the CI-worker entrypoint; the Nix sandbox runs only the
fixture and cohort checks.

The Nix checks `nix-build-adapter-fixtures` and `nix-build-adapter-cohort` run
the fixture-only tests and verify the repo-pinned Nix executable inside the Nix
sandbox. A real build is deliberately outside that sandbox because the sandbox
cannot write a nested Nix store; `tests/nix-build-live.sh` is the host rail.

`cargo test --all-features` also selects the live `nix-live` suite and the Pijul
`native-fixtures` targets. Outside `tests/nix-build-live.sh` and the Pijul Nix
check they lack the realized cohort/job source or `KILN_PIJUL_EXE` and fail by
design; use the documented workspace gate or run the matching host rail.

### Cohort refresh

When the root nixpkgs lock changes, run
`nix build path:$PWD#checks.x86_64-linux.nix-build-adapter-cohort -L`. The
check fails if the pinned `nix`/`nix-store` paths, Nix version, or BLAKE3 no
longer match `pkgs.nix`. Refresh the Rust constants, the Nickel profile, the
documentation, and the evidence together; never update only one surface. The
cohort check is the guard, and the live rail must be rerun after the refresh.

## Rebuild check

`rebuild_check = true` is an explicitly reviewed, off-by-default profile option.
After the exact admitted `.drv` is realised and its outputs are rooted, the
adapter runs one more bounded command:

```text
nix-store --realise --check -K DERIVATION
```

The check uses the build's timeout, stdin, stdout, stderr, poll-interval and
teardown limits. `-K` keeps a differing rebuild beside the realised output; the
observation never replaces the build's products or GC roots.

Each expected output gets one typed observation:

- `matched` — the rebuild compared equal to the stored output;
- `differed` — the rebuild differed and Nix retained `<path>.check`, which is
  recorded as `retained_path`;
- `not_checkable` — no comparison was possible, for example a substituted or
  fixed-output path, or a refused, timed-out, cancelled, output-limited or
  ambiguous check. This is neither a match nor a difference.

Every observation carries the exact output name and path plus the BLAKE3
identity of the bounded check log. A check failure never rewrites the successful
build outcome, and a check that cannot run is not reported as a difference.

The honest wording is: **rebuilt once on this host, matched or differed**. This
is not a reproducibility claim, a gate, required CI, or a statement about other
builders, hosts, or independent rebuilders. Diffoscope integration is not part
of this option; retain a differing output and compare it outside Kiln.

## Non-claims

Passing these checks proves only that this adapter, exact executable, exact
`.drv`, exact expected output paths, local store inputs, GC-root retention, and
native realisation roots were admitted. It does not prove build correctness,
sandbox completeness, derivation provenance or source trust, output content
correctness, cache truth, reproducibility, or release readiness.
