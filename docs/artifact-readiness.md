# Artifact-readiness benchmark

Issue #59's consumer is the maintainer deciding whether artifact layout is worth
optimizing. The maintenance owners are the hosted-cache and Nix-adapter
maintainers. This is an opt-in local measurement rail, not a production feature,
a telemetry backend, VM/session runner, or evidence of a performance improvement.
No benchmark, build, test or formatter was run during implementation.

## Decision declared before measurement

The one proposed optimization is **reduce regular-file materialization overhead
by changing archive layout**. No optimization is implemented by this change.
`benchmarks/artifact-readiness/cohort.ncl` declares the decision and bounded
workloads before a runner starts effects. A later candidate is **go** only when:

- each comparable warm materialization cell has at least **30 complete samples**;
- nearest-rank p95 decreases by both **20%** and **5 ms**;
- exact runner, input/workload, provider/profile and cache-state cohorts match;
- output semantic identities match, with no failed/incomplete attempts or phase
  attribution violations;
- independently reviewed same-work and complexity evidence is accepted; and
- measured retained-byte growth is **zero or less**.

Otherwise comparison publishes **no-go** with reasons. A baseline without a
candidate/review is **insufficient-evidence**, not a speedup or an optimization
recommendation. Three samples are a smoke cohort, not decision evidence. The
analysis exposes medians/range/ECDF at any positive sample count, p95 only at 20
complete attempts, and p99 only at 100. Quantiles use nearest rank. Failed,
incomplete and absent records retain their statuses/errors and are never
imputed as zero-time successes. Distinct runner/workload/profile/cache/delay
cells are never silently pooled.

## Prerequisites and smallest safe real cohort

Run from the exact Kiln checkout under Linux. Required tools are Python 3,
Nickel, `b3sum`, `jj`, and an **already built** hosted example with the opt-in
feature. The parent/integration owner updates the shared Cargo.lock dependency
list once: the feature uses the already pinned `nix = 0.30.1` resource API to
measure calling-thread user/system CPU safely. Normal service builds do not
enable this feature. No shared registry/module/flake change is needed.

After coordinated integration, build and check separately from measurement:

```console
cargo test --locked -p kiln-hosted --features cache-readiness-bench readiness
cargo build --locked -p kiln-hosted --features cache-readiness-bench --example artifact-readiness
python3 scripts/test_artifact_readiness.py
nickel export --format json benchmarks/artifact-readiness/cohort.ncl
```

These invalid fixtures must each fail export (a successful export is a failure):

```console
nickel export --format json benchmarks/artifact-readiness/negative-sample-bound.ncl
nickel export --format json benchmarks/artifact-readiness/negative-file-bound.ncl
```

The **smallest safe cohort command**, after prerequisites, is:

```console
python3 scripts/artifact-readiness.py \
  --output /run/user/1555/kiln-issues-readiness-small \
  --hosted-binary target/debug/examples/artifact-readiness \
  --workload many-files --samples 1
```

Use the actual account's UID and an existing owned parent directory; the output
leaf must not exist. This runs one empty-owned-cache miss, then one exact-input
warm hit into a fresh destination, with real hosted `restore` and `publish`.
It needs no network, credentials, shared GC, service restart or remote mutation.
The runner never compiles the harness. Output artifacts remain for inspection;
there is no automatic deletion or cache GC outside its newly owned directories.
A nonzero exit reports provider/incomplete/attribution failure, not a slower
successful result. Owned commands have a 180-second deadline, bounded output,
and process-group teardown on timeout/interruption. Nix daemon work is not an
owned child and is never killed or restarted by this runner.

The minimum **control** cohort also selects the one-file workload, then repeats
in another fresh output root with a deliberate materialization delay:

```console
python3 scripts/artifact-readiness.py \
  --output /run/user/1555/kiln-issues-readiness-controls \
  --hosted-binary target/debug/examples/artifact-readiness \
  --workload one-file --workload many-files --samples 3
python3 scripts/artifact-readiness.py \
  --output /run/user/1555/kiln-issues-readiness-delayed \
  --hosted-binary target/debug/examples/artifact-readiness \
  --workload many-files --samples 1 --delay-phase materialization --delay-ms 100
```

The injected delay occurs **inside actual hosted restore materialization**. Raw
observations identify its phase and milliseconds; `restore` is an inclusive
parent and also grows, but `execution` receives no injected delay and there is
no compiler in this workload. The analysis checks the marker's phase and lower
elapsed bound and rejects an injection assigned to execution or omitted from an
observed requested phase. A cold miss never materializes an entry, so that phase
and delay are absent there. Rust tests additionally exercise real restore with
unchanged native key/content identities, as well as invalid/nested scopes and
provider-error preservation. Synthetic analysis tests are not runtime evidence.

For sufficient baseline evidence repeat controls with `--samples 30`; p99 needs
`--samples 100`. Keep resource contention, runner affinity and filesystem fixed.
No command drops kernel pages: **cold means empty owned application cache**, not
cold disk/kernel pages. Full CPU and mount facts are retained so tmpfs and disk
results cannot masquerade as equivalent storage. Run roots under `/run` avoid
the constrained shared `/tmp`; no global Nix configuration is changed.

## Optional real Nix file/symlink/shared-reference cohort

The hosted cache format deliberately rejects symlinks and dependency-reference
entries. Such hosted cells are recorded **absent**, not measured as a faster
lower-work substitute. The Nix rail supports those workloads with actual
`builtins.derivation`, `nix-instantiate`, `nix-store --realise --dry-run`, rooted
`--realise`, `nix path-info`, `nix-store --dump` and `nix-store --restore`.

Select an **existing immutable** Bash executable and coreutils store root from
the operator's reviewed cohort; there is no nixpkgs download, arbitrary shell
command input or substitute fallback. Their store identities, generated
expressions, exact derivations/output paths, NAR hashes, path info and commands
are retained. The shared-fanout graph has four distinct branches pointing to one
shared derivation, not four independent copies of that dependency.

```console
python3 scripts/artifact-readiness.py \
  --output /run/user/1555/kiln-issues-readiness-nix \
  --nix-builder "$NIX_BASH" --nix-coreutils "$NIX_COREUTILS" \
  --nix-build-directory /run \
  --workload one-file --workload many-files --workload many-symlinks \
  --workload shared-fanout --samples 1
```

`NIX_BASH` must name `/nix/store/.../bin/bash`; `NIX_COREUTILS` names its existing
store package root. A local-only Nix daemon must be available, with no remote
builders, and the account permitted to build this local cohort. The operator
must establish that prerequisite; this runner cannot attest daemon host policy.
The generated expression uses only these existing
tools and bounded repository-owned scripts. Per-command Nix configuration sets
`min-free = 0`, no substitution, empty builders and one local job; `NIX_REMOTE=daemon`
prevents ambient remote-store selection. `--nix-build-directory` explicitly selects
an existing sandbox-accessible native build parent and binds it into the cohort.
Do not use the private evidence root under a `0700` home/XDG runtime directory:
the sandbox builder cannot traverse it. The daemon owns its temporary children;
no ancestor permissions are changed. The runner never changes nix.conf, invokes
GC, removes shared store objects or fetches an input.
Nix realization retains indirect roots inside the output directory. Existing
host daemon settings, sandbox/build authorization and immutable tool
availability are prerequisites, not authorization to weaken host policy.

Each Nix sample gets a new derivation namespace and then a warm realization of
the **same** exact derivation. Content bytes remain fixed across file/symlink
controls; NAR bytes naturally differ and are measured, not equated with payload
bytes. Dependency references add real graph work and are separate cells. NAR
restore is actual host materialization, not guest hydration. No boot, VM or
remote/object-tier result is invented. A full default workload cohort also
includes `larger-bytes` (262144 versus 65536 payload bytes).

## Observable phase boundaries

| Provider | Recorded phase | Boundary and limitation |
| --- | --- | --- |
| Hosted | `queue_admission` | Actual synchronous cache-owner lock wait, not scheduler queue inference |
| Hosted | `evaluation_preflight` | Existing key/input hashing and absent-output checks; repeated occurrences summed per attempt |
| Hosted | `store_open_manifest_validation` | Actual store open, ownership/manifest checks; includes internal store lock/pruning where provider does so |
| Hosted | `lookup_transfer_decode` | Actual local store lookup/read/JSON decode and provider validation; internals are coalesced |
| Hosted | `manifest_validation` | Existing explicit envelope verification |
| Hosted | `materialization` | Actual verified fd-relative `restore_local`; decode/validation inside that consumer remain included |
| Hosted | `setup` | Cold fixed-byte product creation or warm readiness no-op, explicitly separate from the identical checksum program |
| Hosted | `execution` | Same fixed-byte read/check/hash program on cold and warm products; no compilation claim |
| Hosted | `publication_collection` | Actual successful owned product collection |
| Hosted | `cache_save_publication` | Native cache publication; retry/fencing/order unchanged |
| Hosted | `restore`, `publication` | Inclusive native call parents; **not additive with children** |
| Nix | `evaluation_preflight`, `planning` | Real evaluation and exact derivation dry-run |
| Nix | `realization_execution_import_publication` | Native realization coalesces builder execution/import/store publication; no fictitious split |
| Nix | `metadata_lookup` | Real path-info/manifest facts |
| Nix | `publication_nar_export` | Actual uncompressed NAR serialization, not network download |
| Nix | `materialization` | Native NAR restore to a new owned host path |
| Nix | `execution` | Explicit count/content/symlink verification of materialized output |

Elapsed monotonic nanoseconds are separate from user/system CPU nanoseconds.
Hosted CPU is **calling-thread** usage via safe `getrusage(RUSAGE_THREAD)`.
Nix command CPU is sequential command-child usage **excluding Nix daemon and its
builders**; the verification phase labels its Python-process CPU separately.
Do not call those command-client numbers total build CPU. Remote/object tiers,
network download, decompression, admission outside the observed lock,
guest-hydration and boot are unavailable/absent rather than zero-time records.
The observation feature adds no production config/environment switch; without
it the wrapper directly executes the same provider operation. Keys, source
checks, bounded verification, lock order, retry/fencing, cleanup prerequisites
and successful-publication ordering are unchanged.

## Raw retention and independent analysis

A run retains the original/exported typed cohort, predeclared decision,
repository revision plus BLAKE3 identities of the actual source files,
executable and native tools (with tool versions), hardware/affinity/mount facts,
command arguments/stdout/stderr/exit,
workload counts, native restore/publication facts and keys, failure records,
per-phase raw samples, and independently reproducible `analysis.json`.
`source_identity` in a comparison cell names the workload/input semantics;
actual implementation/binary identities remain in each run's metadata.
Nix cells additionally bind native tool and immutable builder/coreutils identities
into `profile_identity`. Their exact unnormalized output manifest and store paths
are retained separately from a semantic identity: reference names are normalized
only after verifying actual branch targets, shared dependency identity and bytes.
This permits same-work comparison across fresh derivation namespaces without
discarding the native graph evidence.

Run analysis independently without executing a provider:

```console
python3 scripts/artifact_readiness_analysis.py \
  /run/user/1555/kiln-issues-readiness-controls/samples.jsonl
```

A later candidate comparison requires its raw JSONL, the **original** retained
`cohort.json` and an operator-reviewed JSON object containing
`complexity_review_accepted`, `same_semantic_work`, and
`retained_byte_growth`. Do not manufacture review acceptance from a timer.

```console
python3 scripts/artifact_readiness_analysis.py baseline/samples.jsonl \
  --candidate candidate/samples.jsonl --policy baseline/cohort.json \
  --review reviewed-complexity-and-retention.json
```

Comparable cells include full runner/workload/provider/profile/cache/delay/input
identity. The comparison retains median and p95 shifts plus both ECDFs. It does
not normalize away changed work, silently remove failures or make a go claim
from smaller retained evidence. No runtime receipt or performance claim exists
until the parent executes and retains the coordinated cohort. External remote
provider or host-policy qualification requires separate operator approval; this
rail neither requests nor simulates it.
