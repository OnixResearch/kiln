# Kiln operator guide

## Local reference run

1. Enter the development shell:

   ```console
   nix develop path:$PWD
   ```

2. Run the standalone fixture:

   ```console
   cargo run -p kiln-cli -- demo
   ```

3. Run all checks:

   ```console
   cargo test --locked --workspace
   cargo clippy --locked --workspace --all-targets -- -D warnings
   cairn validate --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"
   cairn traceability coverage --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json" --json
   cairn release-readiness --root .
   nix build path:$PWD#checks.x86_64-linux.octet --no-link -L
   nix flake check path:$PWD --no-write-lock-file -L
   ```

   `cairn release-readiness` consumes two checked-in gate fixtures.
   `fixtures/agent-completion/` binds the maintained repository gate commands
   to the current generated policy hash; regenerate its command and selection
   hashes whenever those commands or the policy change.
   `fixtures/preserves-canonicalization/corpus.json` binds canonical Preserves
   values to their BLAKE3 hashes; run
   `cairn preserves-corpus update --path fixtures/preserves-canonicalization/corpus.json --execute`
   after adding or changing a case.

Run credential-isolated acquisition separately from a normal operator build: existing Cargo/Nix caches and credential helpers can conceal an inaccessible source input. Use a fresh temporary HOME/source cache with no Git/SSH credentials or global Git configuration, and acquire all direct and transitive sources before claiming the public quick start is reproducible. Consult `LICENSES.md` before combining adapters into a distributed executable.

For release preparation, bind actual command results to the exact validated source payload and retain failures. The original files under `evidence/` are historical publication evidence, not current check receipts. See `hosting-and-recovery.md` for the boundary between local preparation and separately authorized publication.


## Status

Status has three separate facets:

- readiness reports holds, delays, dependencies, and provider-local capacity;
- execution reports dispatch and provider observations;
- outcome reports the canonical attempt result.

Status is a rebuildable projection. It cannot mutate journal state.

## Observation projection

`kiln-observe-core` is the pure read-only projection layer for operator surfaces.
It admits a typed profile, an exact state revision, and an explicit event or log
window supplied by a shell, then returns deterministic bounded views with
`kiln.observe.view.v1` identities. It provides no transport, CLI, control, or
authority; the HTTP shell is `kiln-adapter-http-observe`, and the Iroh shell
remains a separate change. See [observation projection](observation-projection.md)
and [HTTP observation](http-observation.md). The iroh leg is
`kiln-adapter-iroh-observe`; see [iroh observation](iroh-observation.md).

`kiln-dashboard` is the local operator surface over the same projection. It
admits a canonical Kiln journal document or a canonical SQLite event store,
verifies every record's event identity by recomputation, replays the record
stream canonically, and renders the bounded run, attempt, and event views as a
deterministic terminal report or a self-contained offline interactive HTML page.
The same surface is reachable as `kiln dashboard`. HTML controls search, filter,
and sort the admitted run pages and select embedded run and attempt details;
they never fetch more history or trigger CI. Inline trusted JavaScript keeps
local filter and selection state in the URL fragment across watch reloads,
while a no-JavaScript fallback retains the readable admitted report. It reads
only the files the operator names, loads no external resources, and performs no
effect. A property-based terminal rail drives the packaged binaries under a
pseudo terminal and checks the rendered report and the operator CLI against
properties derived from the staged inputs and the checked-in profiles. The same
rail has a browser specification for the HTML report and its local controls.
See [operator dashboard](observation-dashboard.md).

## Logs and follow

A cursor binds one exact stream and byte offset. A read reports gaps when retained data no longer contains that offset.

A completed follow reports the final cursor and terminal stream state. Truncation does not change an attempt outcome.

## Wait

A wait request binds an exact selection and one named predicate. A client deadline changes only the client result.

The supported predicates are `all_terminal`, `any_terminal`, and `all_successful`.

## Capacity and readiness

Global tenant fairness runs before provider-local capacity admission. Capacity classes do not select workers or calculate tenant shares.

The shell supplies time and capacity observations. The core does not read a clock or poll a provider.

## Controls

An admission hold stops new dispatches in its scope. It does not pause a running attempt.

Cancellation, pause, and resume require an exact active attempt. They also require the provider capability.

Kiln does not support arbitrary process input as a standard control.

## SQLite event store

`kiln-adapter-sqlite` pins `rusqlite 0.40.2`, `libsqlite3-sys 0.38.2`, and bundled SQLite `3.53.2`.

The profile uses immediate transactions, WAL, and `FULL` synchronization. Each transaction commits accepted events, stream revision, and outbox records together.

Before retention, write a snapshot at the exact retention boundary. Replay rejects a retained history without that snapshot.

Use `backup_to` only with a new destination. The adapter runs SQLite integrity checks and records the backup BLAKE3 identity.

Schema changes require an admitted migration edge. Unknown or partially applied versions stop service admission without destructive repair.

## Recovery

Restore the latest committed journal snapshot. Then classify each outbox lease.

Reconcile expired or ambiguous effects before redispatch. Do not claim exactly-once provider execution.

## Isolated Patchgate review

<!-- r[impl kiln.patchgate.profile] r[impl kiln.patchgate.nonclaims] -->

`kiln-patchgate-review` produces isolated review evidence. It is not Cairn's
proposal/design/tasks gate and never runs `accept`, `revert`, `watch` or a source
mutation command. Kiln maintainers own the adapter and repeatability rail;
Patchgate owns native review/merge policy. A reviewed Lattice shell can invoke
this CLI and retain its receipt without receiving merge authority.

### Provision and bind the inputs

- Install Patchgate from
  `git@github.com:brittonr/patchgate.git` revision
  `97c1216fe13000d9dcbb97f70ddd5423180eea2c`, native report schema 1. Private
  acquisition is operator-managed and is not part of normal Kiln builds/tests.
  The repository has no flake; install the packaged derivation into the user
  profile directly, clearing any configured remote builder:

  ```console
  git clone git@github.com:brittonr/patchgate.git /path/to/patchgate
  git -C /path/to/patchgate worktree add --detach /tmp/patchgate-pin \
    97c1216fe13000d9dcbb97f70ddd5423180eea2c
  nix profile install --impure --option builders '' --option secret-key-files '' \
    --expr 'let pkgs = import (builtins.getFlake "nixpkgs") { }; in
             import /tmp/patchgate-pin/nix/package.nix { inherit pkgs; }'
  ```

  The package builds the patched Pijul `1.0.0-beta.24` binary alongside
  `patchgate`, so the wrapper's `PATH` already selects the matching Pijul.
  Later upstream revisions add tests that do not pass on every host filesystem;
  installing this exact pin keeps the acquisition reproducible. Verify the
  installed binary identity before use; the exercised build was BLAKE3
  `c7f9ad4bff9ca55ca57fab6df9aa018981792fa40a8ca3a90d97fefb338c72e5`.
- Keep its patched Pijul executable/database cohort separate from
  `kiln-adapter-pijul`'s beta.11 library profile. The native smoke below used
  Patchgate's patched beta.24 executable; that does not establish compatibility
  with the old library profile.
- Apply `config/patchgate-review.ncl` to operator-owned settings and export the
  result as `profile.json`. The positive fixture under `config/fixtures/` shows
  the shape, not deployable identities. Supply the absolute program/repository,
  executable BLAKE3, provider ID, explicit environment, process/collection limits,
  exact initial main/inbox states, trusted native configuration and policy.
  Preserve native SHA-256 values; do not replace them with Kiln hashes.
- Use an immutable executable path and trusted runtime closure. The adapter
  hashes the bounded executable before dispatch, but does not attest its source,
  transitive tools, or prevent replacement of an operator-writable path between
  hashing and execution. Operators own credentials, model endpoints and Nix
  policy. Only profile-declared environment variables reach the child.
- Prepare `intent.json` with `effect_id`, `pipeline_revision` and `candidate`.
  The candidate contains `id`, `ordered_changes`, `dependencies`, `policy_id`
  and `frozen_sources` as `{change, snapshot}` entries. Every ordered change
  binds a full native Pijul change reference, the initial main state reference,
  an admitted frozen source snapshot, the same pipeline revision and the selected
  collaboration-provider ID. Empty main uses native state `null` and target
  reference ID `empty`. Include the entire selected prerequisite closure;
  undeclared extra changes fail admission. Duplicate frozen-source keys fail.

The planner computes canonical candidate, policy, native-input and idempotency
identities without reading the repository or dispatching Patchgate. Source
snapshot facts remain caller-admitted; review does not prove that they describe
the evaluated filesystem.

### Plan, run, retain and reconcile

```console
cargo build --locked -p kiln-adapter-patchgate --bin kiln-patchgate-review
kiln-patchgate-review plan --profile profile.json --intent intent.json > plan.json
kiln-patchgate-review review --profile profile.json --plan plan.json > receipt.json
kiln-patchgate-review reconcile --profile profile.json --plan plan.json --receipt receipt.json > reconciled.json
```

Use the installed binary or `cargo run --locked -p kiln-adapter-patchgate --bin
kiln-patchgate-review --` when it is not on `PATH`. The root flake exports the
same executable as the `kiln-patchgate-review` app.

The fixed native invocation is `review --dry-run --json` with explicit full
`--change` selections and profile-bound repository/channels. Automatic dependency
expansion is disabled: every authorized prerequisite must be explicit in the
bound candidate, so an undeclared prerequisite cannot trigger a model review.
Bounded Exec supplies null stdin, cleared environment, bounded stdout/stderr,
deadlines, cancellation and process-group teardown. SIGINT/SIGTERM request
cancellation; CLI cancellation exits 130. Other failures exit nonzero and must
not be consumed as review rejection or approval.

A successful exit means the evidence was admitted, **not** that every change was
accepted. Inspect `observation.reviews`: rejected, escalated, blocked, waiting,
pending, removed, reverted and unknown stay distinct. Accepted, rejected and
escalated decisions require fresh decided items and matching records; a prior
terminal ledger state alone is not evidence of a completed new review.
Model-assisted acceptance requires complete coverage when bounded windows are
reported; a rejecting/escalating window can legitimately stop early. Operational
stops and partial evidence cannot become acceptance. The pinned producer can
emit an empty `provenance.requests` vector for a fresh short model review. Kiln
preserves that absence and makes no unavailable prompt/schema digest or verified
model-weight claim; it validates request evidence whenever the producer supplies it.

The `kiln.patchgate.review.v1` receipt retains the exact native JSON bytes as a
string alongside their BLAKE3 identity, the request identity and normalized
observation. Keep it access-controlled: reports can contain private paths,
messages and model evidence. Reconciliation validates that exact prior receipt
against the plan without inspecting the executable or dispatching again. A
moved target, changed configuration/profile/policy, different effect, or altered
report needs new evidence. This is applicability checking, not a signature or
authenticity claim. Review never grants acceptance or release authority.

### Inspect retained receipts in the workspace

The run inspector's **Reviews** tab reads the run's retained reports and selects
only documents whose schema is `kiln.patchgate.review.v1`. For each receipt it
shows the preserved effect, candidate, candidate-root, pipeline-revision,
policy, provider-profile, native-binding and report identities, the request
identity, the recorded time and source revision, one row per change with its
normalized decision, the producer's `isolated` and `authorizes_acceptance`
claims, and the verbatim native report.

Decisions stay distinct: rejected, escalated, blocked, waiting, pending,
removed, reverted and unknown are never folded into "passed". A run with no
receipt says so explicitly; unrelated reports remain under **Tests & coverage**.
The view is read-only, never dispatches Patchgate, and grants no review, merge,
gate or release authority. Upload the receipt JSON as a `review` report to
retain it for the run.

### Retained receipt directory

The workspace's **Reviews** page lists `kiln.patchgate.review.v1` receipts from
one explicitly admitted directory; it never scans elsewhere. Apply
[`config/web-reviews.ncl`](../config/web-reviews.ncl) to an operator-reviewed
`{ label, directory }` record and pass the exported JSON with `--reviews` on
both `check` and `serve`.

```console
nickel typecheck config/web-reviews.ncl
nickel export --format json --output "$KILN_REVIEWS_PROFILE" "$KILN_REVIEWS_NCL"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE" --reviews "$KILN_REVIEWS_PROFILE"
```

`/api/reviews` returns the label, each receipt's name, byte count, BLAKE3,
effect/candidate/report identities, per-decision counts and the producer's
`isolated` and `authorizes_acceptance` claims; `/api/reviews/NAME` returns the
verbatim document. Non-receipt entries, symlinks, oversized files and documents
with another schema are listed under `skipped` with a reason, never as empty
success. The directory is server-owned; names are bounded and traversal is
rejected. Readers are authenticated viewers/operators; workers are denied.

### Produce one receipt outside a run

`scripts/patchgate-review-job.py` builds the profile and intent from live native
states, computes one archive identity per pending inbox change, runs
`kiln-patchgate-review plan` and `review`, and writes the receipt:

```console
python3 scripts/patchgate-review-job.py \
  --target /path/to/pijul-repository \
  --patchgate "$(command -v patchgate)" \
  --adapter "$(nix build path:$PWD#kiln --no-link --print-out-paths)/bin/kiln-patchgate-review" \
  --pijul "$(dirname "$(readlink -f "$(command -v patchgate)")")/pijul" \
  --out patchgate-review.json
```

The script refuses an inbox that is behind main or has no pending change, and it
never records, applies, merges or accepts. Its exit is the adapter's exit;
successful evidence admission is not review acceptance. The receipt is only
visible in the Reviews tab when a run uploads it while active, because report
upload binds the run, node and attempt exactly.

### Real producer repeatability

In the development shell, with `PATCHGATE` naming the pinned installed program,
`PIJUL` its patched executable and `ADAPTER` the built Kiln CLI:

```console
python3 scripts/patchgate-review-native.py \
  --adapter "$ADAPTER" --patchgate "$PATCHGATE" --pijul "$PIJUL" \
  --output /tmp/kiln-patchgate-native.json
```

The rail creates a temporary repository, HOME and signer. It checks real
documentation acceptance, credential-header rejection, protected-path escalation,
exact reconciliation and refusal of stale/tampered/duplicate inputs. It
independently compares the complete original channel inventory and every original
channel state, work files/directories and gate audit state, including dirty/untracked
data. No caller signer or model credentials reach review; all three cases make zero model requests.

The retained run is in
`.cairn/changes/add-patchgate-review-adapter/evidence/native-smoke.json`.
The adjacent `verification.json` records the local checks and the separate
traceability/release-readiness hold: the new requirement IDs remain in the active
change, outside the accepted-spec registry. No acceptance, sync or archive is
implied by implementation verification.

`evidence/boundary-smoke.json` in the same change records a separate throwaway
run through the installed producer with a local deterministic model-protocol
fixture, not a live model: a short model review retained its genuinely absent
request digests, an undeclared prerequisite made no model-port call, and a
historical operational-stop ledger entry was refused as fresh review evidence.
It does not qualify model accuracy, conflict-resolution safety, reusable final
merged snapshots, live model weights, remote delivery, Nix verification,
deployment or release. Dry-run may call a configured model and create temporary
keys or Nix store results in other scenarios: source-nonmutating is not globally
effect-free, sandboxed or hermetic. See `LICENSES.md` for linked Bounded Exec's
AGPL obligations and separate executable provenance responsibilities.


## Nix job discovery

`kiln-adapter-nix-eval` discovers jobs; it does not realize derivations, authorize
substitution, or change canonical CI state directly. Keep the stock
`NixEvalProfile::pinned` cohort and default root flake unchanged unless explicitly
selecting the separate `NixEvalProfile::onix` executable profile. Core crates
retain provider-neutral discovery facts and no Nix/Onix implementation dependency.

### Explicit Onix selection and provisioning

`onix-nix-eval-v1` is an optional `x86_64-linux` discovery profile, not a replacement
for stock Nix. Its acquisition boundary is the separately locked
`runtime/onix-eval-cohort` flake, including an exact onixpkgs SSH review-source
input that may require read credentials. Normal root development/package and
stock discovery do not acquire it. A stock request carrying Onix requirements is
denied before external effects; missing artifacts or drift never authorize a
stock, ambient evaluator, plugin, or impurity fallback.

With `ONIXOS_CHECKOUT` naming a checkout containing the frozen fixture Git object,
run from the Kiln root in the normal `nix develop path:$PWD` tooling shell:

```console
scripts/import-onix-discovery-fixture.sh "$ONIXOS_CHECKOUT"
nix flake archive --no-write-lock-file path:$PWD/runtime/onix-eval-cohort
nix build --no-write-lock-file path:$PWD/runtime/onix-eval-cohort#evaluator path:$PWD/runtime/onix-eval-cohort#plugin path:$PWD/runtime/onix-eval-cohort#manifest --no-link -L
nix eval --raw --no-write-lock-file path:$PWD/runtime/onix-eval-cohort#lib.jobset
nix build --no-write-lock-file path:$PWD/runtime/onix-eval-cohort#checks.x86_64-linux.cohort --no-link -L
nickel export --field profile config/nix-eval-jobs-onix-profile.ncl
```

The importer verifies the exact committed `fixtures/kiln-wasm-discovery/flake.nix`
and preserves it verbatim at `runtime/onix-eval-cohort/jobset/flake.nix`; it refuses
different existing bytes. See the [immutable source receipt](../runtime/onix-eval-cohort/jobset.source.json)
and [exact runtime paths, hashes and manifest contract](nix-eval-jobs-adapter.md#frozen-source-and-measured-runtime).
Provision all selected closures on the actual discovery host and retain their
GC roots; a path on a development host is not worker provisioning. Source
acquisition is not discovery, and the artifact check is not the live Kiln rail.

The Nickel export selects closed profile facts, not a CLI launcher or direct
Rust profile JSON. Provision an operator-owned absolute `home_base` and
`gc_root_base`, then compose the exact `NixEvalRuntimeBinding`, finite limits
and an explicit `OnixSourceBinding` containing the immutable source reference,
flake-byte BLAKE3, root selector and platform. Call
`NixEvalProfile::onix(runtime, limits, source_binding)` and
`NixEvalAdapter::new`. Requests must select `onix-nix-eval-v1`,
`Some(onix_cohort()?)` and exactly match that source binding. The imported
conformance fixture selects `jobs.x86_64-linux` and `x86_64-linux`; the pilot
below independently binds its real consumer jobset. Features are exactly
`flakes`, `nix-command`, `wasm-builtin`; the source binds the exact Nickel WASM
plugin and `evalNickel` entry point through a local NAR-pinned
`builtins.fetchTree` import, not `--impure` or a network provisioning fallback.
The source-defined `nix_wasm_init_v1` handshake is not a guarantee of a stable public ABI.

After provisioning, exercise the real evaluator, observation admission, task
mapping and GC-root handoff through Kiln:

```console
cargo test --locked -p kiln-adapter-nix-eval --test onix_live exact_onix_discovers_nickel_jobs_maps_dependency_and_releases_roots -- --ignored --exact --nocapture
```

Retain the actual result bound to the exact source and measured runtime. The
[integration qualification receipt](../.cairn/archive/1970-01-01-add-onix-nix-evaluator-cohort/evidence/verification.json)
is the evidence boundary, not the presence of this command. The rail expects
real Nickel seed/report jobs, their dependency, an independent `broken` job error,
stock rejection and root lifecycle checks. The adapter measures evaluator,
manifest, plugin, seven evaluator-linked Nix libraries and source with bounded
no-final-symlink opens before new discovery; it does not provide hostile-code
confinement. Preserve distinct provider failure, timeout, cancellation and unknown
outcomes rather than treating a zero exit or partial JSONL stream as completion.
Persist every task handoff, call `handoff_gc_roots` with the durable task IDs, then
`cleanup_gc_roots`; premature cleanup is denied and cleanup failure stays visible.

Discovery does not establish general Onix/stock compatibility, source trust,
cache truth, build/output correctness, reproducibility, deployment, publication,
or release authority. Review [license provenance](../LICENSES.md#optional-onix-discovery-provenance)
before distributing the fixture or runtime cohort; wrapper `NOASSERTION` is not
a permissive license grant.

## Isolated maintained-cohort OnixOS pilot

`scripts/onixos-ci-pilot.py` runs one fresh, private operation through the
maintained Aspen host/extension, a real Lattice workflow, the explicit Onix
evaluator and `kiln-onixos-worker`. `kiln-onixos-pilot` authenticates the
operator request and persists existing core commands/events; HTTPS observation
is rebuilt from that canonical journal, not a constructed fixture state.
Kiln maintainers own the runner, source-binding contract and repeatability rail.
Operators own acquisition, worker isolation and the selected runtime cohort.

Provision the normal `kiln` package (which installs both pilot binaries), the
separately pinned `kiln-aspen-host`, its matching extension, the reviewed Lattice
producer and the optional Onix evaluator/plugin cohort before execution. Use a
frozen Kiln source tree, not a changing checkout, for retained qualification.
`nix develop path:$PWD` supplies Python and the normal Rust/Nickel tools; it does
not provision the external runtime cohort or grant remote mutation authority.

Supply an operator-owned Nickel input record admitted by
[`config/onixos-ci-pilot-inputs.ncl`](../config/onixos-ci-pilot-inputs.ncl).
Every executable is an explicit absolute path. The consumer, WASM shim and
plugin are separate immutable NAR-pinned trees. `nixpkgs_url` must exactly match
the consumer's **root input edge** in `flake.lock`, not an arbitrarily named
`nodes.nixpkgs` entry. A different selected shim/plugin requires
`producer_substitution_reason`; the receipt retains the original locked
`onix-wasm` entry alongside that explicit substitution. Drift never selects a
fallback. The Radicle RID, exact published revision and actor describe genuine
operator ingress source facts; they do not pretend that the independently
selected OnixOS snapshot was published as that revision.

With `PILOT_SOURCE`, `PILOT_INPUTS`, `PILOT_NICKEL` and `PILOT_ROOT` naming the
frozen Kiln source, admitted input record, exact bootstrap Nickel executable and
a new private scratch prefix respectively, run each real case:

```console
for case in success build-failure evaluation-failure; do
  python3 "$PILOT_SOURCE/scripts/onixos-ci-pilot.py" \
    --inputs "$PILOT_INPUTS" --nickel "$PILOT_NICKEL" \
    --root "$PILOT_ROOT-$case" --case "$case" || break
done
```

Existing scratch roots are refused. Each run has finite process, output,
request, journal and recovery bounds, fresh signing/UCAN/TLS credentials and
private sockets/state. The Lattice worker's temporary directory is privately
bound through Bubblewrap to avoid sharing its pathname-based temporary state;
this is not a hostile-workload confinement claim.
The UTF-8 `root/eval-roots/<64-byte BLAKE3 hex>` path must fit Kiln's
128-byte provider-reference limit. Choose a short scratch prefix for every case;
unrepresentable root identities are rejected during profile admission, before
evaluator directories or processes are created.

Preparation discovers the real launcher, activation script and home-file
derivations generated by the consumer's inventory and Nickel `home-tools`
service. A fresh BLAKE3 build identity is bound into the inventory's `git_user`
setting, changing the real generated configuration and its transitive outputs.
Only external toolchain dependencies are realised during preparation; their
transitive derivation and output closures must exclude all three consumer
identities. Immediately before authenticated dispatch, every selected consumer
output must be absent both physically and from native Nix path metadata.
Authenticated CI then constructs the complete selected launcher closure through
the existing no-substitution, no-remote-builder Nix adapter. Success verifies
the generated Git configuration and exact activation/home-file closure members.
It does not execute activation, install profile wrappers or mutate a home.
`provisioning.json`, `before-dispatch.json` and `verification.json` retain this
boundary. An already realised consumer output is rejected, never deleted.

The build-failure case adds a typed file target beneath the real `.gitconfig`
file and requires the actual home-file builder's conflicting-target diagnostic.
A failed builder may leave an unregistered partial directory: evidence records
physical presence separately from native registration and never treats that
directory as a successful product. The fresh pre-dispatch check still rejects
any pre-existing physical consumer output.

The deterministic preparation-policy regressions can be repeated independently:

```console
nix build path:$PWD#checks.x86_64-linux.onixos-pilot-preparation --no-link -L
```

The success case must produce canonical `Success` and exact retained output
roots. The genuine failing derivation must produce `DomainFailure`; the
explicit evaluator control must retain its native diagnostic, produce
`ProviderError` and never create a build plan. Every case exercises tampered
and expired request denial, duplicate delivery before and after completion,
host restart while the independently owned workflow is still pending, and
reconciliation of the original operation with exactly one provider invocation.
Roots survive durable result/journal handoff, then explicit release is repeated
to establish idempotence. UCAN HTTPS probes verify actual run/attempt views and
their identities, and require wrong-holder, wrong-target and replay denial.
Derivation-root cleanup requires a nonempty handoff authorized by the existing
core decision. Failed or unknown discoveries keep their root observation;
they do not invent a downstream handoff. Qualification also rejects terminal
worker errors emitted after a durable result, and requires completed discovery
to discharge its authorized derivation-root release.

Retain `cohort.json`, admitted public profiles, the signed request's public
facts, native discovery/build results and diagnostics, canonical journal,
`verification.json` and `retirement.json`. Never retain private keys or
invocation/bearer credentials. The runner stops its processes and retires
private credential files on success or failure. A receipt is valid only for
its measured source/input/executable/profile cohort. It establishes neither
autonomous forge delivery, production state/status mutation, deployment,
source trust, general exactly-once execution, reproducibility nor release
eligibility. See the [license map](../LICENSES.md) before distribution.

The [historical launcher-only qualification](../.cairn/archive/1970-01-01-qualify-maintained-onixos-ci-pilot/evidence/verification.json)
binds the actual `Success`, `DomainFailure` and `ProviderError` cases to frozen
source, executable and input identities, with complete public command records
and an implementation-byte manifest. Its native repository readiness
conformance passes with `onix_release_ready = false`; this is not production
readiness or release approval.
The [full-closure qualification](../.cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/verification.json)
retains the new construction evidence separately. Its frozen runner/projection
identities and before/after consumer observations bound that stronger build
claim without extending deployment, publication or release authority. A second
success uses the same immutable source with a fresh, previously absent consumer
closure; it does not substitute cached consumer outputs for construction.


## Aspen native runtime

Radicle trigger intake requires an explicit runtime selection. Keep the direct
`--runtime lattice` path for a reviewed Lattice endpoint. Select
`--runtime aspen --aspen-profile /absolute/aspen-runtime.json --aspen-socket /absolute/aspen.sock`
only for the exact deployed Aspen v2 profile and endpoint. Both selections also
require `--profile /absolute/radicle-workflow.json`. A missing or incompatible
Aspen endpoint never authorizes direct Lattice fallback. GitHub's reviewed
Lattice path is independent of this Radicle runtime selection.

The native host command, required bounds, profile binding, state layout, and
recovery procedure are in
[`runtime/kiln-aspen-host/README.md`](../runtime/kiln-aspen-host/README.md).
The state root must already exist. Keep the admitted profile and extension
executable unchanged across recovery; preserve all operation-ledger,
native-journal, callback-values, and host-state components. Never remove an
uncertain operation to permit redispatch. Reconcile it through the exact
persisted Lattice request instead.

The root workspace's source imports and the standalone native host's private
dependency cohort are separate acquisition boundaries. A passing root check is
not a native-host build or a hosted-runtime receipt.

For isolated local conformance after acquiring the standalone cohort, run from
the repository root. The host requires its `nightly-2026-05-26` toolchain (or the
equivalent Nix-owned host check), not the root stable toolchain:

```console
cargo +nightly-2026-05-26 test --locked \
  --manifest-path runtime/kiln-aspen-host/Cargo.toml --features test-fixture --test hosted
cargo test --locked -p kiln-aspen-extension --test hosted
```

The host scenarios run the real Aspen native service and the real Kiln callback
core in a test-only process wrapper, with temporary machine-local durable stores
and a synthetic Lattice protocol peer. They cover terminal acknowledgement,
replay and restart, pre-effect denial, accepted uncertainty, bounded observation,
and profile drift. The extension scenarios separately launch the production
callback executable and check valid and malformed frames and authority denial.
These checks use no production credentials or live service, and do not prove
live Lattice execution, sandboxing, deployment readiness, or global durability.

## GitHub CI adapter

Kiln maintainers own the GitHub adapter, its typed configuration, and the
repeatability rail (`github-protocol-fixtures`, Nickel checks, and the existing
workspace checks). Operators own repository authority, secret provisioning,
reviewed Lattice workflows, host isolation, HTTPS ingress, and deployment.
Nothing in the local fixtures records a live deployment or a remote status write.

### Credentials and webhook

1. Select the real repository owner/name (lowercase in the job profile) and its
   numeric GitHub repository ID. The ID is required; a repository rename or
   replacement is not permission to accept a different repository.
2. Provision a high-entropy webhook secret and a separate API bearer token using
   SecretSpec `0.19.0`. Use a fine-grained token restricted to the repository:
   **Contents: read** for private HTTPS Git acquisition and **Commit statuses:
   write** for publication (including read access for reconciliation), or an
   equivalently scoped GitHub App installation token. Public source may omit
   the source token. The service still requires the status token. Token issuance,
   expiry, and renewal are operator responsibilities; Kiln does not mint tokens.
3. For each secret, supply a `SecretReference` with `program_path` (absolute
   SecretSpec executable), `program_blake3` (the executable's actual BLAKE3),
   `project_root` (absolute directory containing the declared SecretSpec
   project), `profile`, `provider` (name or `null`), and `secret_key`.
   Webhook/API/source references are not secret values. Kiln invokes the pinned
   executable with `get KEY --profile PROFILE [--provider PROVIDER] --reason ...`
   under a cleared environment. Choose a provider usable without ambient shell
   credentials; declare and provision it in that project. Never store tokens
   in Nickel, JSON, workflow argv, URLs, or CI environment. Keep the API and
   source credentials inaccessible to the executed repository command.
4. In GitHub repository **Settings → Webhooks → Add webhook**, select an
   operator-owned **HTTPS** Payload URL ending in `/github/webhook`, content
   type **application/json**, the same webhook secret, and individual **Push**
   and **Pull requests** events. Keep TLS certificate verification enabled.
   Publish only an operator-managed HTTPS reverse proxy; forward the exact body
   and signature/delivery/event headers to `127.0.0.1:8787`. Do not expose the
   loopback HTTP listener directly or rewrite JSON before HMAC verification.

Protocol references: GitHub's [creating webhooks](https://docs.github.com/en/webhooks/using-webhooks/creating-webhooks),
[validating webhook deliveries](https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries),
[commit status REST API](https://docs.github.com/en/rest/commits/statuses?apiVersion=2022-11-28),
and [GitHub App installation authentication](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-as-a-github-app-installation).
Kiln uses HMAC-SHA256 over original body bytes and pins the REST request version
to `2022-11-28`; it publishes commit statuses, not Checks API check runs.
Push deletion, closed PRs, and non-executable events do not start jobs. PR
`opened`, `reopened`, and `synchronize` can start jobs; fork execution is denied
by the supplied Nickel contract. A signature does not establish delivery
freshness, source trust, or safety of executing repository contents.

### Reviewed job and exact Lattice route

Keep operator configuration outside the conformance fixture tree. The builders
`config/github-job.ncl` and `config/github-ci.ncl` each return one runtime JSON
object, not an envelope containing a `profile` field. Apply them to real inputs:

- Job input: `owner`, `repository`, absolute `program_path`, string-array `args`,
  string-map `environment` (use `{}` unless reviewed), and `source_secret`
  (`null` or the source-only SecretReference). The command is fixed reviewed
  argv, not webhook-supplied shell text. Prefer an immutable executable path.
- Service input: `owner`, `repository`, numeric `repository_id`, absolute
  `replay_database_path`, `webhook_secret`, `status_secret`, and `workflow`.
  The complete `Workflow` contract is exported from
  `config/github-contracts.ncl`: it requires the Unix socket, allowed peer user
  IDs, transport/adapter byte and deadline limits, policy/profile/platform IDs,
  exact graph nodes/edges, controls, and graph/log bounds. Each node requires
  `id`, `kind`, `effect_class` (`build` or `test`), `adapter_kind`, and the actual
  64-hex `operation_identity`. Pause and resume are unsupported.

Define `job` and `service` in the operator-owned
`/etc/kiln/github-deployment.ncl` with those required fields and actual reviewed
values. In the checkout root, create two operator-owned export expressions
(adjust relative imports if storing these elsewhere):

```nickel
# github-job-export.ncl
(import "config/github-job.ncl") (import "/etc/kiln/github-deployment.ncl").job
```

```nickel
# github-ci-export.ncl
(import "config/github-ci.ncl") (import "/etc/kiln/github-deployment.ncl").service
```

Export the applied expressions explicitly:

```console
nickel export --format json github-job-export.ncl > /etc/kiln/github-job.json
b3sum /etc/kiln/github-job.json
nickel export --format json github-ci-export.ncl > /etc/kiln/github-ci.json
```

The deployment file is operator input, not a file generated by the repository.
Protect exported configuration and its parent directories against worker writes.
The job identity is BLAKE3 of the **original exported JSON bytes**, including
whitespace and the trailing newline, not canonical JSON or a Nickel source hash.
Re-exporting or changing even whitespace requires a newly reviewed identity.

Use a pinned Lattice runtime implementing producer requirement
`lattice.workflow_interchange.shell_success`: exchange shell commands must exit
zero before the node can succeed, failed dependencies must prevent downstream
effects, and skipped nodes must not carry execution receipts. Older runtimes
that expose a nonzero shell exit as completed data are not CI-compatible even
when their wire schema matches. This is a producer execution requirement;
updating Kiln's contract import alone cannot repair an older running producer.

1. In the pinned Lattice application, import and review a saved shell workflow.
   Its mapped shell command must invoke the installed helper using the exact
   exported file and its actual 64-hex digest:
   `kiln-github-job --profile /etc/kiln/github-job.json --identity DIGEST`.
   Here `DIGEST` means the output of `b3sum` above, not a literal value to deploy.
   Pin the executable/profile/argv in the saved operation and record the actual
   resulting operation and saved workflow revision identities.
2. Configure a sorted workflow-exchange handler route binding the exact consumer
   contract profile identity and graph identity to that saved workflow name and
   revision, and the exact ordered consumer-node → Lattice-node map. Use
   `effect_authority = "route_exact"` and
   `source_binding = "shell_environment"`. Every mapped node must be a shell
   node; the saved workflow must not predefine the three source keys.
   Use the producer's `contracts/workflow-exchange-handler-profile.ncl` and
   `docs/workflow-exchange-application.md` at the pinned Lattice revision, not an
   invented Kiln-side handler schema. A changed operation, saved revision, graph,
   or mapping requires a newly reviewed matching route.
3. Run the installed Lattice application's
   `lattice --config CONFIG workflow-exchange serve --profile HANDLER_PROFILE`
   with the matching socket path, peer user IDs, transport limits, and capacity.
   Lattice injects only the admitted fixed `LATTICE_SOURCE_PROVIDER=github`,
   `LATTICE_SOURCE_REPOSITORY=owner/repository`, and exact 40-hex
   `LATTICE_SOURCE_REVISION` values into mapped shell nodes. See also
   [the workflow contract](lattice-workflow.md), request schema
   `lattice.workflow-request.v2`.
4. The helper acquires only that exact commit via GitHub HTTPS, with no SSH
   transport or ambient credential fallback. It validates the bounded snapshot,
   rejects symlinks, gitlinks, and `.git` path components, materializes a fresh
   temporary checkout without Git metadata, and executes reviewed argv there.
   Acquisition, transfer, snapshot, command, output, and teardown bounds remain
   finite; credentials and inherited environment are not forwarded to the
   command. Bounds and temporary directories are **not a sandbox**. Run jobs
   under a separate worker identity and operator-provided host/container/VM
   isolation. Even same-repository PR code may be hostile; enabling forks needs
   a separately reviewed policy and stronger isolation, not a changed fixture.

The service builder fixes loopback ingress, path, status context `kiln/ci`, denied
forks, explicit non-claims, and conservative named bounds. Rust admission remains
authoritative for graph semantics and cross-field limits. Concrete inputs under
`fixtures/github/{service-profile,job-profile}.ncl` are synthetic conformance
fixtures only; their repository ID, hashes, commands, and paths are not deployment
values. Never export them to `/etc/kiln` as production configuration.

### Service, status, and recovery

The default Nix package installs both binaries. Run them directly or via the Nix
apps (`nix run path:$PWD#kiln-adapter-github -- ...` and
`nix run path:$PWD#kiln-github-job -- ...`):

```console
kiln-adapter-github --profile /etc/kiln/github-ci.json
kiln-adapter-github status --profile /etc/kiln/github-ci.json --run "$RUN_BLAKE3"
kiln-adapter-github reconcile --profile /etc/kiln/github-ci.json --run "$RUN_BLAKE3"
```

Use the actual `run_identity` returned by an admitted webhook's HTTP 202 JSON as
`RUN_BLAKE3`. Admission commits to SQLite before acknowledging; execution occurs
outside the acknowledgement path. Exact duplicates retain the same immutable
candidate/pipeline run; a delivery ID reused with changed bytes is rejected.
`maximum_connections` bounds concurrent admitted HTTP sockets (queued plus
active) in a fixed worker pool, not the lifetime number of deliveries. The
builder selects 16; runtime and Nickel admission cap it at 64. The service
continues accepting deliveries until shutdown. Arrange operator process
supervision and retain the same durable database across restarts.

The database is bound to the exact admitted service-profile identity, including
secret references, limits, and workflow. Changed profiles cannot silently reuse
an existing database. Keep its WAL/SHM consistent during backup, retain the
profile and job bytes plus digests and Lattice route/revision alongside recovery
records, and protect the database from worker mutation. Record limits fail
closed: there is no silent eviction or automatic retention reset. Operators
must preserve replay protection when planning archival/migration; deleting or
replacing a full database is not a safe retry mechanism.

`status` reads local persisted state without remote mutation. `reconcile`
performs bounded recovery/observation and read-only lookup of ambiguous prior
status writes; it does not grant redispatch or blind POST retry authority. It
may publish a newly ready status intent, so it still requires API-write authority.
Dispatch reservations interrupted by a crash recover the original exact request.
Missing acceptance, exhausted polls, or ambiguous observations remain
unknown/unresolved, never inferred success or permission to execute again.
Pending and terminal status POSTs reserve their exact writes before mutation;
an ambiguous response is reconciled against exact commit/context/run/state
records, not reposted. Publication failure cannot change the canonical CI
outcome. Outstanding status intents are processed in order: a newly terminal
result cannot hide an earlier ambiguous pending-status reservation. That
reservation receives one bounded read-only reconciliation before the later
status is considered. The running service can publish statuses only with real operator
authority; the local check does not establish that authority or exercise a live
GitHub write.

For repeatability, retain exact config exports and hashes, workflow/route
identities, selected commit and admitted tree identity, local run status, and
actual check output tied to the exact source revision. The focused
`nix build path:$PWD#checks.x86_64-linux.github-protocol-fixtures` check enables
`protocol-fixtures` solely for isolated loopback/local-source scenarios; normal
workspace packages do not enable those test transports. Combine it with the
Nickel and unchanged workspace checks before making local compatibility claims.
Do not turn local conformance into a deployment, source-trust, confinement,
workflow-correctness, or release-readiness claim.

The retained [local runtime evidence](../evidence/github-ci-local-runtime.ncl)
imports the [raw observations](../evidence/github-ci-local-runtime.json) and binds
the exercised executables and selected producer files by BLAKE3. It covers exact
source success/failure, restart duplicate suppression, and lost-status-response
reconciliation against local fixtures. It also records scoped Lattice-wide gate
results; it does not bind a clean producer release or prove deployment.

## Iroh remote workflow execution

`kiln-adapter-iroh` carries the existing Lattice workflow exchange between a
controller-local Unix proxy and a worker-local Lattice socket:

`Kiln workflow port -> private Unix proxy -> authenticated Iroh -> worker -> Lattice`.

It adds transport, not another executor. Dispatch, exact recovery, observation,
controls, logs and reconciliation keep their native identities and existing Kiln
admission. The separate read-only Iroh observation plan is not implemented here.
The registry cohort pins Iroh `0.95.1` under ALPN `kiln.workflow.v1`, retaining
the workspace's Rust 1.88 toolchain.

### Enrollment and profiles

Generate one stable identity on each host in an operator-owned private directory:

```console
kiln-adapter-iroh keygen --key /etc/kiln/worker.key
kiln-adapter-iroh keygen --key /etc/kiln/controller.key
```

Run the respective command on its own host. Each prints only its public key.
Keys are exactly 32 raw secret bytes, created mode 0600 without overwrite.
Loading rejects symlinks, nonregular files, wrong lengths and group/world access.
Do not commit keys, place them in the Nix store, or put their bytes in profiles.

Apply `config/iroh-workflow.ncl` to reviewed operator inputs and export each
result as JSON. Required inputs are `schema`, `iroh_version`, `alpn`,
`socket_path`, `bind_address`, `allowed_peers`, `allowed_user_ids`,
`relay_urls`, `transport_limits`, `contract`, and `exchange_timeout_ms`.
Use the exact schema `kiln.iroh-workflow-profile.v1` and cohort/ALPN above.
`config/fixtures/iroh-workflow-inputs.ncl` shows the complete shape with
synthetic enrollment and paths; it is not a production configuration.

Each reviewed wrapper invokes the one-argument builder exactly as the positive
fixture does, with its own operator input file. Export the worker and proxy
wrappers before CLI validation:

```console
nickel export --format json /etc/kiln/worker.ncl > /etc/kiln/worker.json
nickel export --format json /etc/kiln/proxy.ncl > /etc/kiln/proxy.json
```

- Worker `allowed_peers` enrolls controller public keys; proxy `allowed_peers`
  pins the selected worker key. Enrollment grants workflow execution authority
  for that exact native contract profile, not merely read access.
- Worker `socket_path` is its existing local Lattice endpoint. Proxy
  `socket_path` is a new private controller-local listener; it must not exist
  at startup. `allowed_user_ids` admits the actual local consumer UID, not an
  assumed fixture UID. The proxy socket is mode 0600; its parent directory and
  service user remain operator-controlled.
- `contract` is the exact native Lattice capability profile already selected
  by the Kiln consumer. The worker rejects any different profile before opening
  Lattice. Configure the producer's exact saved workflow route and authority as
  described above; the bridge cannot repair a missing or stale route.
- Empty `relay_urls` disables relays; discovery and implicit default relays are
  disabled. Optional relay URLs must be explicit HTTPS URLs. Bind addresses and
  public dial hints remain deployment inputs, never execution authority. The
  other address family is bound only on loopback, not an ambient wildcard.
- Frame/log limits and read/write windows use the existing native transport
  profile. `maximum_connections` bounds concurrent bridge exchanges (at most
  64), not bridge lifetime. Configure the actual Lattice server's finite
  connection budget independently. `exchange_timeout_ms` bounds the complete
  bridge exchange; all declared deadlines are finite, at most 300000 ms.

### Start and select the transport

```console
kiln-adapter-iroh validate --profile /etc/kiln/worker.json
kiln-adapter-iroh worker --profile /etc/kiln/worker.json --key /etc/kiln/worker.key
```

Save the worker's single public startup `EndpointAddr` JSON line to a reviewed
`worker-endpoint.json` on the controller. It carries public identity and address
hints only. Its endpoint ID must match proxy enrollment.

```console
kiln-adapter-iroh validate --profile /etc/kiln/proxy.json
kiln-adapter-iroh proxy --profile /etc/kiln/proxy.json --key /etc/kiln/controller.key --remote /etc/kiln/worker-endpoint.json
```

Wait for the private proxy socket, not just its network startup line. Select that
socket in the existing GitHub, Radicle, or other Lattice consumer configuration;
no new CI API, workflow schema, provider ID, or fallback executor is introduced.
The default Nix package installs the binary; `nix run .#kiln-adapter-iroh -- --help`
is the corresponding app.

Lost, malformed, timed-out or cancelled exchanges close without a synthetic
success or retryable result. The existing Unix consumer retains
`UnknownAfterWrite`; reconcile with the original exact `RecoverDispatch`, never
blind ordinary redispatch. SIGINT/SIGTERM stop owned exchanges and await QUIC
endpoint draining; the per-exchange timeout does not interrupt owned teardown.
Graceful proxy shutdown removes only its own socket inode. Changing
enrollment requires restarting the immutable profile and does not cancel work
already accepted by Lattice. Controls and logs retain the selected producer's
capabilities: the current real Lattice handler rejects pause/resume and retains
fixed terminal diagnostic chunks, not arbitrary shell stdout.

Kiln maintainers own the adapter and repeatability rail:
`cargo test --locked -p kiln-adapter-iroh`, strict workspace Clippy, and the
Nickel positive/negative fixtures in the Nix check. Operators own peer enrollment,
private directories, Lattice routes/durability, secrets, worker isolation and
deployment. The change-local evidence records actual local execution and restart
recovery through both bridges. Loopback checks do not establish WAN/NAT/relay
availability, sandboxing, provider truth, CI correctness, exactly-once effects,
publication or release authority.

### Bounded combined live CI exercise

The [operated proof](../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json)
uses ordinary default-feature GitHub/job/Iroh binaries, real SecretSpec 0.19.0
file-provider resolution, a reviewed immutable Lattice producer, and two distinct
hosts. No protocol-fixtures build or fabricated webhook satisfies this rail.
The separate worker compiles and runs
`cargo run --offline --locked -p kiln-standalone --example live_ci_probe` from the exact
forge revision. Its assertions exercise unresolved Unknown, Pending to terminal
DomainFailure, and rejection of a later conflicting success. The negative
revision changes the terminal expectation and really panics; it is not a
fabricated provider failure.

The observation delay and stdout belong to the `kiln-standalone` imperative shell,
not the pure `kiln-core` source tree. The existing core-boundary conformance guard
remains enabled. Superseded initial observations are retained under
`evidence/initial-core-location`, not presented as the final qualified job.

Immediate result: real success/failure statuses and recovery of the original
accepted execution after the isolated controller and both Iroh bridges restart.
The Lattice executor and its job remain running. This does not demonstrate
continuation after killing Lattice, exactly-once external effects, public-WAN or
relay availability, arbitrary untrusted-workload confinement, indefinite
deployment, or release readiness.

Kiln maintainers own the typed `config/live-ci-workflow.ncl` and
`config/live-ci-worker.ncl` builders, standalone-shell behavior probe, no-effect
`live_ci_inputs` profile-to-native-input example, and three `scripts/live-ci-*.py`
operator helpers. These provide a repeatable composition and external-observation
rail, not a new CI state authority. The repository operator owns forge/host
authorization, peer enrollment, ingress, workload review, credentials, private
state, and teardown.

Select immutable executables/source and original exported profile bytes from the
change's `evidence/cohort.json` and `evidence/runtime-inputs.json`. The latter
preserves nonsecret `original_utf8` bytes and BLAKE3 identities, not resolved
credentials. Rebuild/review unavailable store inputs rather than substituting an
unrecorded producer. Use the GitHub/Iroh setup above and the controller/worker
helpers' `--help` to provision fresh authorized private roots, explicit peer IDs,
exact routes, and a real SecretSpec provider. Bootstrap guards refuse unrelated
existing roots/users/ports rather than overwriting them. Helpers are scoped to
the recorded validation deployment, not a persistent deployment manager. New
identities, binaries, or authored inputs require fresh exported profiles and
recorded hashes; never repair identity fields by hand.

Copy each required Nix store closure to the host that uses it before starting
the helpers; a path present on the operator workstation or controller does not
provision the worker. For example,
`nix copy --to "ssh-ng://$WORKER_HOST" "$KILN_PACKAGE"` stages the complete worker
package closure. Provision the recorded producer, shell, compiler, vendor, and
SecretSpec closures on their respective hosts as well; verify executable hashes
there against the reviewed cohort. Keep explicit GC roots for the operated
inputs until retirement; an unrooted build output can disappear between setup
and execution.

The replay database is bound to one exact service profile. A changed job/service
uses a fresh replay path while preserving the old database; never rewrite its
profile metadata or canonical outcomes to accept a changed configuration.

The worker helper requires the native socket's live listener to belong to the
owned unit's `MainPID`; a leftover socket file is not readiness. Use `stop lattice`
before `start lattice`. Stop waits for that unit, refuses a still-listening
endpoint, and removes only its owned, non-symlink Unix socket. It preserves the
Lattice store. This endpoint cleanup does not resume a killed executor or justify
rewriting an unresolved run; the recorded recovery case restarts only the
controller and bridges while the original executor remains alive.

Prepare an isolated Jujutsu checkout at the recorded validation base, add the
reviewed positive `live_ci_probe.rs` as a new commit, and create a tracked
`kiln-live-ci-validation-*` bookmark at that commit. Review that its remote is the
authorized repository. After starting the worker's Lattice/Iroh units, controller
proxy/GitHub units, and isolated HTTPS ingress, use the authorized values:

```console
python3 scripts/live-ci-exercise.py create-hook --url "$HTTPS_URL/github/webhook" --webhook-file "$PRIVATE_WEBHOOK_FILE" --output "$EVIDENCE/webhook.json"
python3 scripts/live-ci-exercise.py run --case pass --checkout "$CHECKOUT" --branch "$VALIDATION_BOOKMARK" --hook-id "$HOOK_ID" --output "$EVIDENCE/pass.json"
python3 scripts/live-ci-exercise.py run --case fail --checkout "$CHECKOUT" --branch "$VALIDATION_BOOKMARK" --hook-id "$HOOK_ID" --output "$EVIDENCE/fail.json"
python3 scripts/live-ci-exercise.py run --case recovery --checkout "$CHECKOUT" --branch "$VALIDATION_BOOKMARK" --hook-id "$HOOK_ID" --output "$EVIDENCE/recovery.json"
```

`HOOK_ID` is the real create-hook response. Host defaults are the recorded cohort;
use explicit host options for another authorized pair. Run cases in that order:
fail and recovery create their own exact source revisions. The driver requires
one real worker job per case, matching canonical outcome, Published local intent,
and exact remote status ID. The failing case requests a genuine forge redelivery
and requires the same run without another job. Recovery checks the still-running
executor PID across interruption and retains the accepted execution identity.
Canonical SQLite state is read-only; the driver never rewrites outcomes/statuses.

The exception in `evidence/operator-authority.json` permits temporary copies of an
existing broad operator credential only for this bounded validation, not as
least-privilege production credentials. After proof, retire the new webhook, stop
only the new controller/worker units, use controller `cleanup-credentials` and
worker `retire-credentials`, and remove the private local webhook handoff. Release
only this exercise's GC-root links after its jobs stop; retain protected keys,
SQLite history, and the native Lattice store for audit. Preserve unrelated
services/ingress, main, governance, and the original operator credential.
Exact retirement observations are retained with the proof. Persistent deployment
needs separately provisioned repository-scoped credentials and its own operator
authorization.


## Radicle source-to-build composition

`kiln-radicle-ci` joins the existing broker protocol, verified local Radicle
source, maintained Aspen/Lattice workflow, Nix worker and dedicated status shell.
It does not manufacture an operator signature or weaken the OnixOS pilot's
signed-request contract. Kiln maintainers own this composition and its proof
rail; OnixOS owns service deployment, and operators retain source, signer,
worker-isolation and publication authority.

### Reviewed project and controller

Define operator-owned inputs for `config/radicle-ci-worker.ncl`:

- `source`: absolute local repository path, exact RID, signer node, full
  `refs/heads/...` branch, source profile ID and snapshot bounds;
- absolute private `state_root`, exact evaluator profile/discovery plan, selected
  task alias, exact Nix build profile and build intent;
- the existing explicit Onix profile when needed, never automatic stock/Onix
  selection or fallback. The source template is replaced only with measured
  acquired-source facts; reviewed selectors and cohorts stay fixed.

Apply that builder to the reviewed inputs, export JSON, and hash its **original
exported bytes** with `b3sum`. The saved Lattice shell operation invokes
`kiln-onixos-worker project-run PROFILE_JSON PROFILE_BLAKE3`. Bind the exact
operation/workflow revision through the maintained `route_exact` and
`shell_environment` contract. Lattice supplies only the admitted
`LATTICE_SOURCE_PROVIDER`, `LATTICE_SOURCE_REPOSITORY` and
`LATTICE_SOURCE_REVISION`; do not predefine or substitute them in the saved job.
Do not rewrite the saved operation for each event.

Apply `config/radicle-ci.ncl` to the exact worker-profile path/hash, existing
Aspen socket, status storage root, allowed RID and separate status signer
declaration. Review the exported runtime profile against the actual host.
The controller and project worker need explicitly provisioned access to the
same candidate-local source intent and worker-result paths. Protect profiles,
source intent and canonical journal against untrusted job writes; the local
same-user proof does not establish worker confinement. Source/trigger authority
does not authorize status signing, remote synchronization or production rollout.

Configure the admitted broker integration to invoke:

```console
kiln-radicle-ci --profile /etc/kiln/radicle-ci.json
```

This process reads the deployed broker JSON from stdin and writes only broker
responses to stdout. The first supported shape is a push for the configured RID
and branch. Foreign repositories, branches, malformed revisions and unsupported
patch/event shapes fail before workflow submission. Source acquisition pins one
signed-refs commit and the exact event revision, verifies the bounded source
contract, and rejects symlinks, gitlinks, unsafe paths and drift. A moved branch
does not make recovery select a newer source.

### Recovery and explicit root ownership

`source.json` below is the exact `{ "provider": "radicle", "repository": RID,
"revision": COMMIT }` returned by the admitted source context, not a moving ref.

```console
kiln-radicle-ci reconcile /etc/kiln/radicle-ci.json source.json
kiln-radicle-ci replay /etc/kiln/radicle-ci.json source.json
kiln-radicle-ci observe /etc/kiln/radicle-ci.json source.json
kiln-onixos-worker project-release /etc/kiln/radicle-worker.json PROFILE_BLAKE3 source.json
```

The controller durably reserves canonical start, native ingress and exact source
before submission. Duplicate events load that reservation. Reconciliation uses
the original operation; an interrupted submission or worker import never grants
a fresh dispatch. Invalid worker evidence is rejected before journal mutation.
The original submit reply is retained, changing nonterminal replies replace the
progress slot, and the first receipt of each distinct terminal decision/variant
is retained in order. Bounds remain a fail-closed backstop, not a lifetime poll
counter that prevents later completion.

Completion requires matching native terminal and actual source/worker result,
selected discovery task and admitted build observation. `Pending`/`Unknown`,
missing evidence or conflicts never synthesize success. Exhausting observation,
or having no exact written status, exits nonzero without `Finished`; it must not
turn a pending or successful run into a broker `Finished(failure)`. Resume with
the original profile and source using `reconcile`, not another job invocation.

Canonical terminal state determines the separate revision-bound status plan.
The controller persists that exact intent before signing/publication. An
interrupted or unknown publication uses read reconciliation, not a blind
second write. Current status profiles/actions require the exact source revision;
incompatible historical actions fail closed. See [status authority and
compatibility](radicle-status.md). No command implicitly synchronizes peers.

The worker imports unchanged acquired bytes through the measured Nix executable,
registers its source root before log fsyncs and full store remeasurement, then
reuses existing discovery/build root handoff. Import and root registration are
separate native operations, not an atomic-GC guarantee. Source and successful
build-output roots remain owned until explicit `project-release`; repeat release
is idempotent and cannot remove a substituted root target. Discovery roots keep
the existing durable-task handoff policy.

### Repeat the isolated native proof

Build the current controller, worker, callback and fixture helpers before the
run; do not rebuild mutable executable paths while the proof is running. Prefer
immutable installed tool paths and retain their BLAKE3 identities.

```console
cargo build --locked --offline -p kiln-radicle-ci -p kiln-onixos-worker \
  -p kiln-adapter-radicle -p kiln-aspen-extension --all-features --bins --examples
python3 scripts/radicle-ci-native.py \
  --inputs .cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/success/inputs.json \
  --root /var/tmp/kiln-radicle-source-proof \
  --nickel "$NICKEL" --nixpkgs "$NIXPKGS" \
  --worker "$WORKER" --extension "$EXTENSION" --coordinator "$CONTROLLER" \
  --source-fixture "$SOURCE_FIXTURE" --broker-tool "$CIBTOOL" \
  --secretspec-fixture "$SECRETSPEC_FIXTURE" --radicle "$RADICLE"
```

Use actual selected executable paths, not literal environment-variable names.
The input file supplies the frozen external-tool cohort; the explicit worker,
callback, controller and helper arguments select the current built source.
`CIBTOOL` is the pinned real broker producer, not hand-written protocol JSON.
`SOURCE_FIXTURE` is the `ci_source_fixture` example, and `SECRETSPEC_FIXTURE` is
the isolated status-key helper. The scratch root must be fresh and outside
`/tmp`; provide usable private temporary storage if host `/tmp` is exhausted.
The optional fixture build/switch is never a production signer fallback.

The runner creates an isolated signed Radicle repository and invokes real
`cibtool trigger --message`. Its unchanged saved worker builds exact event bytes
with the output absent before dispatch, survives a controller/host restart with
one invocation, publishes/reconciles matching revision statuses, and produces a
genuine Nix failure for the next revision. A third revision with an unavailable
provider proves bounded broker uncertainty and duplicate suppression without a
final reply or status. Source/build root release, UCAN read-only observation,
exact executable/profile/source identities, command records and private
credential/process retirement are retained in the scratch root.

This proves the declared local producer/source/runtime/build/status composition,
not a live remote Radicle delivery, production source trust, hermeticity,
sandboxing, remote status dissemination, activation, deployment or release.

The [retained qualification](../.cairn/archive/1970-01-01-compose-radicle-source-to-build-ci/evidence/verification.json)
links the exact native cohort, original commands, canonical journals, source and
build observations, rejected attempts, root release and retirement records.

### Native Radicle peer and broker canary

The [live qualification](../.cairn/archive/1970-01-01-qualify-live-radicle-ci/evidence/verification.json)
starts from a genuine native peer update rather than a `cibtool trigger`. The
runner provisions three fresh, separately identified nodes on one host
(publisher, CI and observer), subscribes the installed broker to native events
for exactly one canary RID/branch, and leaves the reviewed controller, project
worker and workflow profile unchanged. It never mutates the operator's existing
Radicle home, repository, signer or services.

```console
nix develop path:$PWD -c python3 scripts/radicle-ci-live.py \
  --inputs .cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/success/inputs.json \
  --nickel "$NICKEL" --nixpkgs "$NIXPKGS" \
  --worker "$WORKER" --extension "$EXTENSION" --coordinator "$COORDINATOR" \
  --radicle "$RADICLE" --radicle-node "$RADICLE_NODE" \
  --broker "$BROKER" --broker-tool "$CIBTOOL" \
  --jj "$JJ" --git "$GIT" --secretspec "$SECRETSPEC" \
  --root "/var/tmp/kiln-radicle-live-$RANDOM"
```

Use the resolved paths from the recorded cohort and a root that does not exist;
`config/radicle-ci-live.ncl` admits only a canonical `/var/tmp/<fresh>`
descendant, the exact branch pair and the pinned store executables. The runner
creates the three node identities, derives the dedicated status seed from the
fresh CI node's unencrypted Ed25519 key, writes a private SecretSpec dotenv
manifest, and never reads an operator credential. Native peers use loopback TCP
with no seeds, relays, default network or public addresses.

Each revision must arrive as an actual native peer fetch. The broker's own
subscription produces the original adapter bytes; the canary retains the exact
`request.json`, attempt replies and canonical journal. For success and
source-controlled failure, completion requires a written dedicated status
intent, explicit signing/synchronization of only
`refs/heads/kiln-status-checkpoint`, and an independent fetch/reduce from
observer storage returning the same RID, revision, run, outcome, object ID and
head. The success case additionally kills the controller host while the worker
and Lattice continue; reconciliation must resume the original operation with no
worker result before completion and exactly one provider invocation. The
unavailable case stops the host before dispatch and must remain nonterminal with
only the `triggered` reply and no status.

The canary releases every successful source/build root, stops all owned
services, and deletes node keys and the generated seed. The retained evidence
records the exact cohort, ingress and replies, journals and status observations,
rejected attempts and retirement. This is a same-host native loopback canary on
trusted reviewed source; it does not establish WAN or distinct-host delivery,
hostile-worker confinement, production deployment, activation or release
authority.

### Distinct-host publisher canary

The [distinct-host qualification](../.cairn/archive/1970-01-01-qualify-distinct-host-radicle-ci/evidence/verification.json)
moves only the publisher node and its source worktree to a second owned host.
The CI node, broker, controller, worker, Aspen host and observer remain local.
The CI node connects directly to the publisher's private tailnet address; SSH
provisions files and owns the remote process but never forwards the Radicle
data port.

Prerequisites: two owned hosts on the same private tailnet with direct TCP in
both directions, the pinned Radicle node, jj and git store paths on the remote
host, and a fresh canonical `/var/tmp` path used on both hosts. A firewall that
blocks LAN inbound is expected; the tailnet must be reachable.

```console
nix develop path:$PWD -c python3 scripts/radicle-ci-live-distinct-host.py \
  --inputs .cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/success/inputs.json \
  --nickel "$NICKEL" --nixpkgs "$NIXPKGS" \
  --worker "$WORKER" --extension "$EXTENSION" --coordinator "$COORDINATOR" \
  --radicle "$RADICLE" --radicle-node "$RADICLE_NODE" \
  --broker "$BROKER" --broker-tool "$CIBTOOL" \
  --jj "$JJ" --git "$GIT" --secretspec "$SECRETSPEC" \
  --root "/var/tmp/kiln-radicle-live-distinct-$(date -u +%Y%m%dT%H%M%SZ)" \
  --remote-host aspen1 \
  --remote-root "<same path>" \
  --remote-address 100.100.103.95 \
  --remote-port 52000 \
  --remote-readiness-seconds 60
```

The runner refuses a non-fresh root, a non-tailnet address, a non-canonical
path or an undeclared host before remote effects. It copies the publisher home
and initial source, starts and seeds the remote node, restarts the local CI
node so its peer table contains the direct remote address, and keeps the
broker/controller/worker unchanged. Each case commits and pushes the exact
source revision remotely; the local worktree supplies the expected output
evaluation. The remote node is seeded after start and after restart.

The retained evidence records the remote host, tailnet address, direct
listener, CI outbound and publisher inbound connections, successful fetches,
original ingress, canonical journals, independently fetched local status,
remote restart/reconnect, root release and remote retirement. The first
rejected attempt is retained as a policy/seed correction.

This is a distinct-host private-tailnet qualification. Status dissemination
remained same-host. It does not establish WAN, hostile-network, hostile-worker
confinement, production deployment, activation or release authority.

### Distinct-host status observer

The [status qualification](../.cairn/archive/1970-01-01-qualify-distinct-host-radicle-status/evidence/verification.json)
moves the observer node and its status storage to the second host while status
object creation, signing and checkpoint synchronization remain local. The local
CI node listens on the private tailnet; the remote observer connects directly,
fetches the status checkpoint ref and reduces it in its own storage with the
pinned read-only `kiln-radicle-ci status-reconcile` command.

Prerequisites: the distinct-host publisher prerequisites, reverse tailnet TCP
to the local CI node, the pinned coordinator and SecretSpec paths on the remote
host, and a fresh shared remote root.

```console
nix develop path:$PWD -c python3 scripts/radicle-ci-live-distinct-status.py \
  --inputs .cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/success/inputs.json \
  --nickel "$NICKEL" --nixpkgs "$NIXPKGS" \
  --worker "$WORKER" --extension "$EXTENSION" --coordinator "$COORDINATOR" \
  --radicle "$RADICLE" --radicle-node "$RADICLE_NODE" \
  --broker "$BROKER" --broker-tool "$CIBTOOL" \
  --jj "$JJ" --git "$GIT" --secretspec "$SECRETSPEC" \
  --root "/var/tmp/kiln-radicle-live-dist-status-$(date -u +%Y%m%dT%H%M%SZ)" \
  --remote-host aspen1 \
  --remote-root "<same path>" \
  --remote-address 100.100.103.95 \
  --local-address 100.110.43.11 \
  --remote-port 52000 \
  --remote-readiness-seconds 60
```

The runner waits for the previous CI control socket to disappear and for
`rad node status` to succeed before connecting peers. It requires the pre-sign
remote read to be absent and the post-sign remote reduction to equal the local
object/head exactly. The unavailable-provider case retains no status.

This is distinct-host dissemination and readback over a private tailnet with
local status authority. It is not WAN, hostile-node isolation, production
deployment or release authority.

## Single-tenant live web service

`kiln-web-service` composes existing Kiln core, Git snapshot and Lattice workflow
ports into a dedicated HTTPS application. The current consumer is Kiln self-CI;
Kiln application/dashboard and provider-adapter maintainers own this composition
and its repeatability rail. The existing `kiln-dashboard` HTML remains a separate
offline, read-only report.

**Current qualification:** the corrected immutable Lattice executable passes the
unchanged strict native exercise: source-bound `Success`, intentional
`DomainFailure`, and `Cancelled` after the actual command starts. Restart keeps
the original execution without redispatch; logs, artifact bytes/hash,
authentication, TLS and exact control checks pass. The measured executable,
source and profile identities and resource retirement are retained in
[the native receipt](../.cairn/changes/prepare-web-service-deployment/evidence/native-corrected/verification.json).
The earlier cancellation misclassification and compiler failures remain
historical evidence, not a verdict on this corrected cohort. Lattice's broader
repository checks and Kiln's separate readiness checks are not all green; this
local qualification grants no production activation or lifecycle acceptance.

The packaged systemd VM rail and supported Onix/NixOS assembly pass for the
frozen inputs retained in deployment evidence. Kiln's complete flake check also
passes. These checks do not exercise the corrected native cancellation cohort
or override the separate failing Cairn release-readiness verdict.

### Install reviewed profiles and authority

Use `config/web-service.ncl` and `config/web-worker.ncl` as the closed typed input
builders. Export operator-reviewed Nickel expressions that apply those builders
to real installed inputs; conformance fixtures are not deployment configuration.
The independently installable `packages.${system}.web-service` contains the binary
and supervised unit; `apps.${system}.kiln-web-service` selects that package.
The default workspace package also contains the binary.

- The service profile declares one registered HTTPS origin/listen address,
  certificate/private-key paths, separate operator/worker token files, an owned
  state directory, allowlisted local Git repositories/references and reviewed
  workflow profiles. Browser enrollment selects only this allowlist; it cannot
  submit paths, commands or replacement providers.
- Token files contain exactly 64 hexadecimal bytes without a newline. Use
  independent random tokens in owned private regular files, mode `0400` or `0600`,
  outside source and worker checkout roots. The HTTPS key is likewise private. Install a
  valid server certificate chain and an explicit worker CA; do not bypass TLS
  validation in a deployed worker.
- The `kiln.web-worker.v2` profile binds the service origin, project, CA/token-file
  references, canonical Git `source_repository` identity, worker-local
  `repository_path`, source snapshot bounds, an owned checkout root, explicit
  executable/argv and environment for each step, and declared relative regular-file
  artifacts. Commands receive no ambient environment or service credential.
  Reviewed commands still retain their worker account's authority: this is not a sandbox.
- Workflow, HTTP, state, log and artifact bounds are positive and explicit.
  Lattice's native serve connection limit counts lifetime exchanges, not just
  concurrent clients; provision a finite budget for polling and worker uploads.
  Exhaustion remains provider uncertainty, never successful completion.

With the reviewed expression and output paths supplied by the operator:

```console
nickel export --format json --output "$KILN_WORKER_PROFILE" "$KILN_WORKER_NCL"
b3sum "$KILN_WORKER_PROFILE"
nickel export --format json --output "$KILN_SERVICE_PROFILE" "$KILN_SERVICE_NCL"
kiln-web-service reviewed-request --profile "$KILN_SERVICE_PROFILE" --project "$KILN_PROJECT"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE"
kiln-web-service serve --profile "$KILN_SERVICE_PROFILE"
```

Retain the original worker JSON and its exact BLAKE3; equivalent reserialization
does not preserve that identity. Register the reviewed request's exact profile,
graph, node mapping and worker operation identity with the existing Lattice
`route_exact`/`shell_environment` handler. Its saved job invokes:

```console
kiln-web-service job --profile "$KILN_WORKER_PROFILE" --identity "$KILN_WORKER_BLAKE3"
```

Lattice supplies the exact `LATTICE_SOURCE_PROVIDER`,
`LATTICE_SOURCE_REPOSITORY` and `LATTICE_SOURCE_REVISION`. The helper verifies that
source context, admits the exact Git snapshot, and executes in an owned temporary
checkout rather than the mutable worktree. Follow the existing
[workflow application contract](lattice-workflow.md) for native handler setup;
profile generation does not install or start it.

### Navigate the live workspace

Overview totals link to retained run filters; the active count opens Operations.
Failing projects appear before healthy projects. Projects places enrollment and
source-bound execution above the inventory; **Select for new run** only selects
the project and never dispatches it.

Run choices show project, outcome, local creation time and a shortened source
revision. Open **Exact source & execution identities** for complete values.
Execution controls remain under a separate disclosure and retain the exact
confirmation and service-side admission. Opening Logs reads the selected stream
immediately; empty output is not success or complete-log evidence.

Evidence and comparison inputs offer bounded retained-run suggestions while
still accepting exact IDs. Comparison suggestions exclude the current run and
other projects; missing suggestions do not establish that no older run exists.
Source requires an explicit repository selection. Changing or clearing that
selection clears stale files and diffs; independent history/tree/file failures
remain explicit. COB and receipt inspection preserves exact selected identities
even when inventory reads fail. Expand complete retained fields when needed;
producer decisions and missing authority values never grant acceptance authority.

On narrow screens, the navigation drawer traps keyboard focus. Escape or the
backdrop closes it; closed navigation is removed from keyboard focus. Page and
run changes move focus to the selected heading. Appearance and density remain
URL-restored, and clearing a session removes private views and suggestions.

Kiln application/dashboard maintainers own these frontend behaviors. Browser
asset overrides against authenticated read-only APIs exercise source candidates
without deploying them; they do not prove the installed service uses those bytes.

### Extend an existing project registry

<!-- r[related kiln.web.projects.onixresearch] -->

Ordinary startup remains bound to one exact service profile. Adding a project is
an explicit offline authority extension, not an implicit migration or a database
reset. Kiln application maintainers own admission and recovery behavior; the
operator owns repository selection, command review, installed routes and activation.

<!-- r[verify kiln.web.projects.onixresearch] -->
The [retained installed-project observation](../.cairn/changes/extend-live-project-registry/evidence/verification.json)
records fourteen additional reviewed committed-ref jobs through authenticated
enrollment and native routes: eight successes and six explicit prerequisite
failures. Original worker/routes, five original runs and retained bytes were
preserved; restart recovered all nineteen run views. These are the recorded
cohort's observations, not execution of the procedure below or qualification of
today's revisions. Existing Kiln self-CI retained its independently pinned cohort.

1. Preserve the original admitted service JSON. Export the next reviewed service
   profile with a nonempty appended project suffix. Preserve every existing
   project in order, including its label, source bounds, workflow and operation
   identities. Preserve all credentials, TLS/service/state roots and resource
   bounds. Only `bounds.max_projects` may increase; it may remain unchanged when
   existing capacity suffices.
2. Export and retain original new-worker JSON bytes and BLAKE3 identities. Prepare
   the reviewed saved workflows and appended exact `route_exact` / `shell_environment`
   graph/node mappings for the native handler. Preserve existing workers, saved
   workflow revisions and routes; changing them is not project extension authority.
   The typed [OnixResearch command catalog](../config/onixresearch-projects.ncl)
   supplies reviewed committed-ref CI commands, not paths, credentials, source
   identities, installed routes or automatic enrollment. Review those bindings
   separately using the existing service and v2 worker builders.
   Jobs select the declared Git ref, not a Jujutsu bookmark or dirty worktree;
   retain each acquired commit even when those local names disagree.
3. Wait until every existing execution is terminal. Stop the service and native
   handler before importing workflows or extending the binding, and back up the
   SQLite database, retained logs/artifacts, original profiles and Lattice store.
   Import the reviewed saved workflows while quiescent. Keep the next profile
   separate until its binding transition succeeds.

```console
kiln-web-service extend-projects \
  --previous-profile "$KILN_PREVIOUS_SERVICE_PROFILE" \
  --profile "$KILN_NEXT_SERVICE_PROFILE"
```

The command requires an existing versioned store, takes the existing owned-state
lock, validates canonical replay and retained evidence, and transactionally
compare-and-swaps only the exact profile binding. It does not rewrite canonical
state bytes, call a provider, enroll projects or create missing service state.
An active owner/run, changed existing authority, stale/replayed transition,
invalid suffix, corrupt retained state or failed transaction rejects the change.
The old profile cannot reopen the successfully extended store.

Activate the matching next service profile and appended handler, then restart
the handler, service and any dependent proxy stopped with the service. Enroll
the new allowlisted projects through authenticated **Projects** intake. Exercise
their real jobs and inspect **Projects**, **Workers**, exact source revisions,
retained output and original histories. CI failures remain failures; enrollment
and a passing bounded job are not whole-repository release qualification.

The [recorded local registry exercise](../.cairn/changes/extend-live-project-registry/evidence/verification.json)
added fourteen projects while preserving the original five Kiln run views and
ten retained files. Eight initial checks passed. Five others could not fetch
SSH dependencies because the host rejected its SSH configuration permissions;
the remaining check received HTTP 404 for its pinned Octet archive. Those six
real prerequisite failures remain visible and do not authorize permission,
credential or dependency-pin changes. The receipt binds exact revisions,
operations, retained logs, installed executable and observed UI surfaces.

### Remote Lattice workers

The live service uses the existing [Iroh workflow bridge](#iroh-remote-workflow-execution).
Set the project's workflow socket to the controller-local proxy socket. On the
worker, point the enrolled Iroh worker at its native Lattice handler socket and
register the exact reviewed graph, saved workflow revision and node mappings.
The worker must reach the service's registered HTTPS origin and trust its CA;
Iroh transports workflow exchanges, not source objects or worker callbacks.

The v2 worker profile separates two identities:

- `source_repository`: the controller's canonical Git repository native identity,
  exactly 64 lowercase hexadecimal characters, without a `b3:` prefix. For this
  local Git source provider it is BLAKE3 of the UTF-8 canonical controller
  repository path bytes, without a trailing newline. It must match the run's
  `repository.native_id` and Lattice's `LATTICE_SOURCE_REPOSITORY`.
- `repository_path`: an explicitly provisioned worker-local Git checkout containing
  the requested commit and bounded source tree. It may have a different absolute
  path and local repository identity. Provision/fetch source separately; the job
  does not fetch or substitute a moving branch. Dirty worktree bytes are not inputs.

Worker admission checks the declared repository against the producer context
before materialization. Callback admission still requires the exact acquired
commit and canonical tree, project, attempt, execution and selected nodes. The
local Git acquisition keeps its own identity; it is not relabeled as the controller
repository. This is an operator-reviewed binding, not repository ownership or
source-trust verification.

**Migration:** v1 profiles and missing bindings are rejected. Re-export reviewed
Nickel inputs with `source_repository`, retain the new original JSON bytes and
BLAKE3, and regenerate/review worker operation identities, graphs and exact native
routes before use. Historical receipts and deployed profiles are not migrated or
requalified implicitly.

Repeat the disposable native exercise with the selected current executables and
the maintained native Lattice source cohort:

```console
python3 scripts/web-service-smoke.py \
  --service "$KILN_WEB_SERVICE" --lattice "$LATTICE" \
  --lattice-source "$LATTICE_SOURCE" --iroh "$KILN_ADAPTER_IROH" \
  --root "$NEW_PRIVATE_SCRATCH_OUTSIDE_TMP" --evidence "$EVIDENCE_DIRECTORY"
```

Omit `--iroh` for direct local execution. The remote mode uses enrolled ephemeral
loopback peers and an independent worker clone; it exercises actual source
commands, dependencies, logs, artifact bytes, failures, planned restart recovery
and cancellation. For restart, it stops the controller before restarting both
bridges, then reconciles the original execution without redispatch. A live lost
exchange retains canonical `Unknown`; this check does not add an automatic
retry or restore a pending control surface after such an outage. It does not
establish WAN/relay availability or distinct-host deployment. `local`/`remote`
adapter labels bind supported capabilities and exact routes; they do not select
per-node placement or split a DAG across hosts.

### Optional signed COB workspace

The live **Work**, **Planning**, **Triage**, **My work & inbox** and **Saved views**
pages use an independently selected `--work` policy. Kiln's web-service maintainers
own this application surface; the immediate consumer is the developer coordinating
source-bound CI work. The existing CI project registry remains execution policy,
not an editable planning-project database.

Apply [`config/web-work.ncl`](../config/web-work.ncl) to an operator-reviewed
record containing exactly `git_directory`, `signer_seed_file` and
`write_principals`. The builder emits `kiln.web-work.v1`. Select an existing,
operator-owned local Git directory, preferably a dedicated store, and provision
a private file containing exactly **32 raw bytes** of service-signing seed.
Hex text, implicit keys and discovered Radicle/operator keystores are not accepted.
The Git directory must be owned by the service UID and not group/world-writable.
Keep the exact signing key and Git history together when backing up or restoring.

```console
nickel export --format json --output "$KILN_WORK_PROFILE" "$KILN_WORK_NCL"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE" --work "$KILN_WORK_PROFILE"
kiln-web-service serve --profile "$KILN_SERVICE_PROFILE" --work "$KILN_WORK_PROFILE"
```

Retain other independently admitted runtime options. Omitting `--work` leaves an
explicit unconfigured surface and never discovers a default store. `check`
admits policy and private seed without opening or modifying the Git repository;
actual requests additionally verify store ownership, native framing, signatures
and replay. No installed store, service configuration or operator key is changed
by the disposable verification procedure below.

An authenticated Viewer or Operator may collaborate only when its **exact
application principal ID** is listed in `write_principals`. This is independent
of CI Operator permission. Unlisted users remain read-only; Workers cannot use
the work API. Same-origin JSON write admission is still required. Removing a
writer denies its new commands while preserving its previously service-signed
history. The signature attests the service-recorded authenticated principal,
not an end-user Radicle signature or user-held key.

Nine version-1 native types under `xyz.onixresearch.work.` retain issues,
projects, milestones, cycles, initiatives, teams, requests, views and personal
attention records. The pure model validates typed relationships, parent/blocker/
duplicate cycles, team-defined statuses, scheduling, authorship and privacy.
Requests preserve original customer/source intake. Comments, mentions, reactions,
descriptions and team/project updates are durable operations, not browser-local
canonical state. Personal subscriptions, read-head markers, favorites and
reminders are private COB fields. The inbox derives attention from the admitted
snapshot and browser time; this does not provide background push/email delivery.

Private views and personal records are owner-controlled at the application API;
they are **not encrypted from Git/store administrators**. View ownership and
visibility are immutable: create a separate shared view rather than exposing a
formerly private history. All work types deliberately stay out of the separate
raw `/api/cobs` browser, which has no principal-aware work-history privacy.

Writes confirm exact values and native heads. Stale heads, changed request
replays and competing/foreign branches fail explicitly; exact duplicate requests
return the original historical result. A signed global sequence and predecessor
operation bind cross-object replay independently of timestamps and object-ID
ordering. The local store uses a cross-process lock and native ref CAS; it is a
single application history, **not automatic CRDT merging or remote synchronization**.
Bounds include 1,024 objects, 128 operations per object, 8,192 total operations,
16 MiB of verified history, 256 KiB per operation/reduced record and 4 MiB per
HTTP response. Exceeding a bound reports unavailable rather than truncated success.

Use Ctrl/Cmd+K for cross-entity search and commands. Work rows support j/k or
arrow navigation, Enter to open, Space to preview, x or checkboxes for selection,
and Shift+F10 for actions. Bulk changes confirm per-object heads, run sequentially
and report partial outcomes; they are not atomic and are never blindly retried.
Named views persist filters, sorting, grouping and list/board layout as COBs.
Explicit URL controls override a named view only when present; opening it from
favorites or the palette restores its saved configuration, including board layout.
The Kiln web-service shell owns this optional collaboration surface and its
native smoke exercise; its consumers are authenticated workspace principals.
Clearing the session erases private snapshots, previews, results and pending reads.

Linked change detail joins only explicit source/revision, patch/review, run,
artifact and Cairn identities. Missing or inaccessible observations remain
unavailable; work completion and recorded provider verdicts grant no merge,
lifecycle or release acceptance. No work action writes native Radicle
issue/patch objects, publishes refs, fetches peers or changes provider authority.

Repeat native persistence, restart, stale/replay, privacy and actual CI/source
joins using the maintained disposable exercise:

```console
cargo test --locked -p kiln-web-service
python3 scripts/web-service-smoke.py \
  --service "$KILN_WEB_SERVICE" --lattice "$LATTICE" \
  --lattice-source "$LATTICE_SOURCE" --work \
  --root "$NEW_PRIVATE_SCRATCH_OUTSIDE_TMP" --evidence "$EVIDENCE_DIRECTORY"
```

Add `--browser-hold` only when driving the browser and writing its evidence marker
before the exercise's bounded wait expires. The exercise creates caller-owned
disposable stores and credentials, uses real native signatures and source-bound
commands, and retires its resources. This is repeatability evidence for the
selected local cohort, not installed-service activation or release qualification.

### Optional read-only COB browser

<!-- r[related kiln.dashboard.cobs.operated] -->
<!-- r[related kiln.dashboard.cobs.operated.scenario.empty] -->

The live workspace's **COBs** view reads explicitly selected local Git
stores, independently of CI project enrollment. It does not search the current
directory, `$RAD_HOME`, a Radicle node, sibling checkouts or remote peers. Omitting
`--cobs` leaves the browser unconfigured; an admitted empty repository list is
configured but empty. Neither case discovers a default store.

Apply [`config/web-cobs.ncl`](../config/web-cobs.ncl) to an operator-reviewed
input record with a `repositories` array. The builder exports schema
`kiln.web-cobs.v1`; each repository has exactly these fields:

| Field | Meaning |
| --- | --- |
| `id` | Stable public local key used by the `repository` query parameter; the Nickel component contract permits at most 100 lowercase ASCII letters, digits, `_`, `.` or `-`, excluding `.` and `..`. |
| `label` | Public display label, 1–128 printable ASCII characters in the authoring contract. |
| `repository_id` | Operator-declared native repository identity, 1–256 printable ASCII characters; this is not the local key or a derived path. |
| `git_directory` | Absolute server-owned Git directory, not a worktree or browser-supplied path; no `..` components. Select the actual bare store or worktree's Git directory explicitly. |

Runtime admission also rejects duplicate local keys, native repository IDs or
Git directory paths. The selected directory must exist as a directory, not a
final symlink, for a read to succeed. This is bounded local observation, not
hostile-filesystem confinement. Public keys, labels and native IDs are disclosed
to admitted readers: do not put secrets or host paths in those fields. Server
`git_directory` paths and filesystem diagnostics are not returned in API JSON.

Supply the reviewed expression and exported JSON paths separately from the
original service profile:

```console
nickel typecheck config/web-cobs.ncl
nickel export --format json --output "$KILN_COBS_PROFILE" "$KILN_COBS_NCL"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE" --cobs "$KILN_COBS_PROFILE"
kiln-web-service serve --profile "$KILN_SERVICE_PROFILE" --cobs "$KILN_COBS_PROFILE"
```

Retain any independently admitted `--access` and `--automation` options on both
commands. `check` admits the policy, not store availability, signatures or
history completeness; inspect actual readback after startup. This runtime policy
does not change the canonical CI `ServiceProfile` identity, worker JSON identity,
CI journal, source enrollment or workflow authority. The packaged deployment
does not implicitly enable it; a separately reviewed service invocation must
pass `--cobs` and provision read access to the selected directories.

The registry exposes three groups of types, with no producer reducer. The six
Cairn decision types are produced by Cairn:

- `xyz.onixresearch.cairn-change`
- `xyz.onixresearch.rfd`
- `xyz.onixresearch.adr`
- `xyz.onixresearch.verification-obligation`
- `xyz.onixresearch.release-decision`
- `xyz.onixresearch.policy-exception`

The three Radicle collaboration types are observed as they exist in local
stores, without translation, lifecycle reduction or Kiln meaning:

- `xyz.radicle.issue`
- `xyz.radicle.patch`
- `xyz.radworks.job`

Refs of any other type are skipped without failing the inventory; an explicit
`type` filter outside this registry is rejected with `cob_type_invalid`.

Authenticated viewers and operators may make bodyless GET requests:

- `/api/cobs` accepts only optional `repository` (local key), `type` (exact
  registered name), `q`, `limit` and `cursor`. Search is case-insensitive substring
  matching over bounded recorded root titles, the root-commit summary, object ID,
  local key, label, native repository ID, type and recorded author. It does not
  search later edits, document bodies or arbitrary action content.
  Ordering is by local key,
  type and native object ID. The default page has 25 objects; `limit` is 1–100.
  Keep filters and limit unchanged when following the returned `next_cursor`.
  Cursor snapshots bind those selections and observed object/ref-tip identities;
  moved refs or changed selections reject stale paging instead of silently
  skipping to another result.
- `/api/cobs/object?repository=KEY&type=TYPE&object=OID` requires all three exact
  selections. `OID` is the native 40-character lowercase hexadecimal root commit
  ID, not the generated Kiln display identity. Missing objects return an explicit
  missing result; malformed identities, unknown/duplicate query fields, bodies,
  browser paths, writes, anonymous readers and worker credentials are rejected.

Inventory returns all admitted public repository keys/labels/native IDs in
`repositories`, selected-store availability and counts in `sources`, all
registered `types`, paged `objects`, filtered `total`, `next_cursor`, `snapshot`,
`complete` and `non_claims`. Each object row carries its exact type, bounded
recorded root title (or root-summary fallback), native object ID and ref tips plus
the root commit's recorded author and time; the browser renders that time in local
time alongside UTC. Titles may come from later numbered actions in the root, not
just action zero. Unavailable or oversized metadata does not make up a title:
the root summary remains the fallback. These are not reduced current titles.
An unavailable
selected store makes the inventory incomplete,
not falsely empty; detail reads fail explicitly on unavailable/malformed stores
or exhausted bounds. Distinguish these from an available store containing no
supported objects. The page restores repository/type/search/paging and the exact
object selection in its URL. A stale selection does not auto-select a replacement;
clearing a session clears private COB views and pending reads.

Native refs are `refs/cobs/<type>/<root-oid>` or
`refs/namespaces/<key>/refs/cobs/<type>/<root-oid>`. The inspector preserves every
observed ref tip, native operation ID, Git parents, recorded author/time/message
and numbered JSON action blob identity/content. Each operation is checked against
its type manifest. History follows operation parents back to the selected root;
parents named by `Rad-Resource` or `Rad-Related` trailers are resource links, not
history edges. Concurrent tips remain observations, not a chosen semantic winner.

The inspector presents content by native type: Cairn change artifacts, RFD/ADR
documents, verification-obligation fields and claims, release candidates and
evidence, policy-exception requests, issue comments/edits, patch revisions/reviews
and native job Request/Run/Finished events. Named artifacts and fields have
separate disclosures; body text preserves its recorded whitespace without JSON
escape sequences. Markdown/HTML and source-supplied URLs remain inert text, not
executed markup, navigable links or fetched resources. Admitted patch-to-source
lookup links retain their separate exact-commit checks.

Exact object identities/ref tips, operation framing and raw JSON remain available
in labelled disclosures. Unknown or malformed action shapes open raw content
instead of guessing meaning. Every recorded operation remains visible in identity
order, including concurrent and superseded claims. A lifecycle, vote or job
outcome is a recorded claim, not current lifecycle state, CI success or acceptance
authority. This presentation path is maintained by Kiln dashboard maintainers.

Current implementation bounds are fixed, not policy knobs:

| Read boundary | Maximum |
| --- | --- |
| Policy JSON / repositories | 262,144 bytes / 64 |
| Each decoded query value / response JSON | 256 bytes / 4,194,304 bytes |
| Scanned refs per store / inventory objects across selected stores | 16,384 / 4,096 |
| Ref tips per object / history operations per detail | 128 / 128 |
| Git parents per operation / tree entries per operation | 128 / 256 |
| Commit / tree / manifest bytes | 65,536 / 65,536 / 4,096 |
| Numbered actions per operation / bytes per action | 128 / 262,144 |
| Root metadata preview bytes across numbered actions / excerpt characters | 8,192 / 180 |
| Retained action bytes across one history / displayed root-title or summary characters | 1,048,576 / 256 |

Raw action JSON and commit metadata are untrusted recorded content, rendered as
inert text. `signature_status = "not_verified"`, `signatures_verified = false`
and `reduced_state = null` are deliberate boundaries: this is not Cairn-reduced
lifecycle state, actor authentication, signed-ref verification, a semantic
conflict verdict, remote completeness, synchronization or publication proof.
Browsing performs no Radicle signing, mutation, fetch, sync or publication and
grants no CI, gate, acceptance or release authority. No repository credentials
or transport authority are admitted by this policy.

Kiln application/dashboard maintainers own this view and its repeatability
rail; operators own store selection, local filesystem authority, policy review,
reader enrollment and deployment. Repeat the persistent authoring checks from
the development shell:

```sh
nickel typecheck config/web-cobs.ncl
nickel export config/fixtures/web-cobs-positive.ncl >/dev/null
for fixture in config/fixtures/negative-web-cobs-*.ncl; do
  if nickel export "$fixture" >/dev/null 2>&1; then
    echo "negative COB fixture unexpectedly passed: $fixture" >&2
    exit 1
  fi
done
cargo test --locked -p kiln-web-service --test cobs
nix build path:$PWD#checks.x86_64-linux.nickel --no-link -L
```

The positive fixture and unknown-field/relative-path rejection fixtures are
authoring checks, not installed-store or successful runtime evidence. For actual
readback, open **COBs** as an admitted viewer/operator, inspect inventory and
one exact native object, follow a returned cursor, restore its URL and clear the
session. Repeat with a deliberately unavailable selected store and verify an
explicit incomplete/error state rather than an empty-success claim. Local
fixtures and browser observations do not qualify deployment or release readiness.

<!-- r[related kiln.dashboard.cobs.population] -->
<!-- r[related kiln.dashboard.cobs.population.scenario.import] -->

The browser shows nothing until the selected store holds `refs/cobs/<type>/<oid>`
objects. To populate it from the repository's own reviewed lifecycle artifacts,
import each active change once, as an explicit offline operator action:

```console
cairn cob import <change> --root . \
  --git-repository "$KILN_COB_STORE" \
  --keystore "$RAD_HOME/keys" \
  --workspace "$KILN_RID" \
  --policy "$PWD/cairn-policy/generated/cairn-policy.json"
```

Preview returns the actor, artifact identities and plan without mutation; add
`--execute` only after reviewing it. Creation signs with that keystore, so the
commit is bound to that Radicle identity. It writes only
`refs/namespaces/<actor>/refs/cobs/xyz.onixresearch.cairn-change/<oid>`, refuses a
change that already has a COB, and performs no publish, sync, announce, delegate
or peer action. The read-only COB browser cannot create these objects. Imported objects
are raw recorded history: signatures, authority and lifecycle reduction stay
unverified by the browser.

<!-- r[verify kiln.dashboard.cobs.population] -->
<!-- r[verify kiln.dashboard.cobs.population.scenario.import] -->
The [retained signed-population observation](../.cairn/changes/show-local-run-time-check-cobs/evidence/verification.json)
under `cob_population` records fourteen imports with the existing operator
identity, one new namespaced ref per object, and duplicate-import refusal.
Pre-existing refs, HEAD, packed refs and store configuration were unchanged.
The live browser listed all fourteen native IDs/tips and opened retained
artifact text; signatures remained unverified. No publication, synchronization,
announcement, delegate change or peer exchange occurred. This is scoped
historical evidence, not authority to import again or proof about newer changes.


Inspect the candidate-bound local outcomes in
`.cairn/changes/add-onix-cob-dashboard/evidence/verification.json`; the commands
and procedure above are repeatability instructions, not a claim that a new
candidate has passed them.

### Read-only source browser

The live workspace's **Source** view reads explicitly admitted local
repositories. It does not discover repositories beside the service, search the
current directory, resolve a remote or check out a working tree.

Apply [`config/web-sources.ncl`](../config/web-sources.ncl) to an
operator-reviewed input record. The builder exports schema
`kiln.web-sources.v1`; each entry has these fields:

| Field | Meaning |
| --- | --- |
| `id` | Stable public key used by `/api/sources/<id>/…`; lowercase ASCII letters, digits, `_`, `.` or `-`, excluding `.` and `..`. |
| `label` | Public display label, 1–128 printable ASCII characters. |
| `kind` | Exactly `git`, `jj` or `pijul`. |
| `path` | Absolute server-owned repository path. For Git this is a worktree or bare repository; for Jujutsu a colocated worktree or a store with `.jj/repo/store/git`; for Pijul a checkout containing `.pijul`. |
| `channel` | Pijul only: the channel whose current state is read. |
| `executable` | Pijul only: absolute path to the `pijul` program. Omitting it makes reads report unavailability rather than silently failing. |

Admission rejects unknown kinds, relative paths or executables, duplicate ids or
paths, `channel`/`executable` on a Git or Jujutsu entry, and a Pijul entry
without a channel. Paths, executables and object-store locations never appear in
browser requests or responses.

```console
nickel typecheck config/web-sources.ncl
nickel export --format json --output "$KILN_SOURCES_PROFILE" "$KILN_SOURCES_NCL"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE" --sources "$KILN_SOURCES_PROFILE"
kiln-web-service serve --profile "$KILN_SERVICE_PROFILE" --sources "$KILN_SOURCES_PROFILE"
```

Retain any independently admitted `--access`, `--automation` and `--cobs`
options on both commands. Omitting `--sources` leaves the view unconfigured; an
admitted empty list is configured but empty.

Authenticated viewers and operators may make bodyless GET requests:

- `/api/sources` returns each configured key, label, kind, channel, availability,
  observed head and revision scope.
- `/api/sources/KEY/log?limit=&cursor=` returns at most 100 revisions per page,
  newest first. `cursor` is a bounded offset, not a revision selector.
- `/api/sources/KEY/tree?revision=&path=` lists one directory (`path` is a
  relative directory path; traversal, backslashes and absolute forms are
  rejected).
- `/api/sources/KEY/file?revision=&path=` returns one file's text when it is
  valid UTF-8 without NUL bytes, otherwise `binary: true` and no content.
- `/api/sources/KEY/diff?revision=` returns the exact commit's unified diff
  against its first parent (an empty parent for a root commit) with file,
  insertion and deletion counts. Truncation is reported, never silent. Pijul
  rejects this route: its CLI exposes no historical diff.

Git and Jujutsu reads resolve an exact 40-character commit id, or the current
head when `revision` is empty, from the repository's Git object store; a
symbolic revision is rejected. Pijul reads run the configured program under
Bounded Exec with fixed argv (`log`, `list`), a cleared environment and a 15 s
deadline. The Pijul CLI exposes no historical blob read, so Pijul trees describe
the current channel state and Pijul files describe the tracked working copy;
`revision` is rejected for a Pijul source and both scopes are labelled.

- `/api/sources/lookup?revision=` reports which admitted Git/Jujutsu sources
  actually contain one exact commit, so a recorded Radicle patch can be opened
  as a real diff. Sources that lack the object are omitted, never guessed.

The view adds client-side entry filtering, an explicit up-one-level control and
newer/older revision steps over the loaded page; all of them change only the URL
selection, never the repository.

Fixed implementation bounds: 64 sources per policy; 100 log entries per page;
512 tree entries; 262,144 file bytes; 262,144 diff bytes; 1 MiB retained program
output; 4 MiB response. Reads never fetch, check out, write a ref, sign, publish or alter CI
state, and they grant no CI, gate, acceptance or release authority. A Jujutsu
store without a Git-visible head, an unavailable Pijul program and an unreadable
repository are reported as unavailable, never as an empty repository.

Repeat the checks from the development shell:

```sh
nickel typecheck config/web-sources.ncl
nickel export config/fixtures/web-sources-positive.ncl >/dev/null
for fixture in config/fixtures/negative-web-sources-*.ncl; do
  if nickel export "$fixture" >/dev/null 2>&1; then
    echo "negative source fixture unexpectedly passed: $fixture" >&2
    exit 1
  fi
done
cargo test --locked -p kiln-web-service --test sources
```

### Supervised deployment

Kiln self-CI is the consumer; Kiln maintains the application and these assets.
The operator owns host selection, account/ACL reservation, TLS identity,
credentials, workflow authority and activation. Assembling a package or system
is not permission to activate it. No asset selects a real host or generates TLS.

#### Product and Onix integration

The dedicated product is `packages.${system}.web-service`. Its unit is installed
at `lib/systemd/system/kiln-web-service.service`; the package installer substitutes
`@out@` in `deployment/kiln-web-service.service.in` with the immutable output path.
The package must contain `bin/kiln-web-service`.

The external module export is exactly:

```nix
onixModules.kiln-web-service = { file = ./deployment/onixos-module.ncl; };
```

Use the reviewed Kiln flake as an existing consumer input. Pass its module set
through Onix's existing provenance route, and resolve its requested package with
an explicit overlay (inside the consumer's existing `onix.mkFlake` arguments):

```nix
moduleInputs.kiln = kiln.onixModules;
overlays = [
  (final: prev: {
    kiln-web-service = kiln.packages.${final.stdenv.hostPlatform.system}.web-service;
  })
];
```

In the existing inventory, bind an instance's `service` to `kiln-web-service` and
`module = { input = "kiln", name = "kiln-web-service" }`. Select exactly one
existing machine in `roles.server.machines`. Required `roles.server.settings`:

- `uid`, `gid`: operator-reserved non-root account IDs (not example host values).
- `profile_path`: absolute, reviewed runtime JSON path; no shell syntax or unit
  specifiers. The module does not create or replace this profile.
- `credential_sources`: four absolute runtime source paths keyed by
  `tls-certificate`, `tls-key`, `operator-token`, `worker-token`. These may name
  operator-provisioned files or files materialized by the consumer's existing
  artifact system. Do not pass Nix paths, secret strings, or store locations.

The module is explicitly NixOS/systemd-specific, not an implementation of a new
Onix backend. It emits private preserved state intent and native systemd leaves
because the portable service provider has no notify-readiness contract. It has
one fixed user/group/unit/state identity and rejects multiple instances or
multiple server machines. No supplementary groups or firewall openings are
created; grant the selected UID only the necessary repository read/traverse and
Lattice Unix-socket access outside this module.

The selected NixOS machine must enable `preservation` with an operator-chosen
`persistentStoragePath`, and the consumer must pass its reviewed
`inputs.preservation` into `mkFlake`. Provision that backing filesystem in the
consumer's existing hardware/storage definition. Onix rejects this module's
preserved state without the adapter; declaring a persistent root alone is not
sufficient. Do not downgrade the state to volatile to bypass admission.

#### Profile and credentials

`config/web-systemd.ncl` accepts the same application input as
`config/web-service.ncl`, **omitting** `tls`, `operator_token_file`,
`worker_token_file`, and `state_directory`. It delegates all application checks
to that builder and binds:

- state: `/var/lib/kiln-web-service`;
- certificate: `/run/credentials/kiln-web-service.service/tls-certificate`;
- private key: `/run/credentials/kiln-web-service.service/tls-key`;
- tokens: `/run/credentials/kiln-web-service.service/{operator-token,worker-token}`.

Export the reviewed application through this helper as JSON at `profile_path`.
The JSON contains references, not credential bytes. Supply real HTTPS origin,
listen address (unprivileged port), repository/source/workflow identities and
bounds through the original builder. The process has no bind capability. Keep
repositories under an accessible read-only deployment path such as `/srv`, and
proxy sockets under `/run`; `/home` is hidden and `/tmp` is private. Provision a
traversable proxy socket parent even if the external provider is unavailable.
Startup admission never performs a live provider exchange.

For the standalone packaged unit, use `/etc/kiln-web-service/profile.json` and
root-owned credential sources below `/var/lib/kiln-web-service-credentials/`:
`tls-certificate.pem`, `tls-private-key.pem`, `operator-token`, `worker-token`.
Pre-provision the dedicated `kiln-web-service` account with the reserved UID/GID
and no login access. A systemd drop-in can replace references for another layout;
clear `LoadCredential=` before replacing the four bindings. Do not change the
credential destination names or bound state without an explicit profile review.

Provision credential sources as root-owned, single-link regular files, mode
0400 or 0600, in a root-owned 0700 directory outside application-writable state.
The artifact owner must materialize these runtime files before start, never
place decrypted bytes in the Nix store or profile. systemd loads the sources into
root-owned **0440 files on a read-only credential mount**, with an ACL granting
read only to root and the service UID. Kiln checks that exact ACL and mount flag
before reading directly; no root helper, permission relaxation or private runtime
copy is needed. Ordinary group/world-readable files remain denied. No-follow,
single-link and private-key byte bounds still apply. Source provisioning remains
a trusted operator/artifact boundary, not a claim of race safety against root.

#### Supervision, rotation and recovery

The actual unit runs `check --profile` before `serve --profile`; preflight does
not mutate state, open listeners or dispatch work. `Type=notify` with
`NotifyAccess=main` waits for initialized TLS/listener/canonical state. HTTPS
`GET /healthz` is minimal and unauthenticated, with configured Host/origin
admission. Readiness is not proof the external provider is reachable.

State is owned by the dedicated identity, mode 0700, and is the only declared
persistent writable path. Source access is read-only. The unit uses no-new-
privileges, no capabilities, private devices/tmp, protected system/home/kernel,
restricted syscalls/namespaces/address families, 1 GiB memory, 128 tasks, 1024
open files and 200% CPU ceilings. Review those bounds against the chosen profile;
use an explicit reviewed drop-in rather than silently widening them. Restart is
on failure with a five-second delay and at most three attempts per minute;
startup has 60 seconds and SIGTERM shutdown 30 seconds before group kill.
External workflows belong to the provider and must not be child processes of
this web-service unit.

Canonical-state ownership is explicitly unlocked after the shell's other fields
close. A descriptor inherited during another thread's fork-to-exec window cannot
retain ownership after that shell is dropped. A second live owner is still
rejected; this does not bypass canonical replay, profile binding or recovery.

Rotate by provisioning replacement source files securely and restarting under
operator authority. Do not delete state to force a restart: durable state binds
the exact profile and recovers original operations without blind redispatch.
Restore the same profile, UID and state after a crash. Changed profiles require
explicit operator review; this helper provides no automatic state migration.

#### Maintained desktop user-service updates

The operated desktop workspace uses the **user** `kiln-web-service.service`,
not the system unit above. Its executable comes from the `kiln-web-service`
element in the operator's Nix profile. Updating it requires explicit operator
authority; do not change profiles, credentials, worker routes or durable state
to make a package start.

Freeze the source revision and build its dedicated `web-service` output. Retain
the current package path, a consistent SQLite backup, private configuration and
retained-file backups, and the exact API run snapshot before activation. Run the
new executable's `check` with every argument from the installed unit's
`ExecStartPre`. Set `new_package` to the built output and `previous_package` to
the recorded installed output. For the current Nix profile CLI:

```sh
nix profile remove kiln-web-service
nix profile add "$new_package"
systemctl --user restart kiln-web-service.service
```

`nix profile add` derives the element name from this package; it has no `--name`
option. The old process remains running during profile replacement. If adding
the new output fails, add `"$previous_package"` before restarting. If activation
fails after replacement, remove the new element, add the previous output and
restart the same user unit. Do not roll an old database snapshot over live state
as part of an executable rollback.

Existing `Requires=` dependencies can restart the identity resolver and tailnet
proxy with the web service. Verify all three units, the unchanged Lattice PID,
CA-verified HTTPS health, exact served asset identities, unchanged unrelated
profile elements, and preservation of the original runs and retained files.
Exercise the served browser without asset overrides. The
[2026-10-28 deployment receipt](../.cairn/changes/improve-lattice-rendering/evidence/deployment.json)
binds the published code, package, source acquisitions, rollback and live checks;
it does not waive lifecycle or release holds.

#### Non-mutating evaluation and isolated proof

Run these after integration, not while sibling changes are in flight:

```sh
nickel export deployment/fixtures/positive.ncl
nickel export config/fixtures/web-systemd-positive.ncl
nickel export --import-path "$ONIXOS/lib" deployment/fixtures/module-positive.ncl
```

Each `deployment/fixtures/negative-*.ncl` must fail export (module fixtures need
that same reviewed Onix import path); so must
`config/fixtures/negative-web-systemd-*.ncl`. They cover unsafe path expansion,
traversal/injection, missing/store/application-writable credential sources,
root/fractional account IDs, insecure origin and accidental state rebinding.
Symlinks/ownership/credential contents require runtime admission, not textual
path fixtures. The synthetic fixture package handle is only an eval argument;
it is never substituted for the real executable in runtime proof.

Build the dedicated package and run its disposable systemd qualification:

```sh
nix build path:$PWD#web-service --no-link --print-out-paths
nix build path:$PWD#checks.x86_64-linux.web-service-deployment --no-link -L
```

Assemble/build the consumer system without activating it. The disposable VM
exercises the packaged unit with real TLS and LoadCredential sources: HTTPS
health, main-PID readiness, UID/mode/state, SIGTERM, restart/recovery and
invalid/missing/private-file rejection. Run the
native workflow success/failure/log/artifact/cancellation rail separately with
the exact corrected Lattice cohort. Neither evaluation nor that isolated proof
claims production activation, source trust, arbitrary-job confinement,
lifecycle acceptance or release readiness.

### Use the live interface

Open the registered HTTPS origin. The page first makes a bounded, read-only
`GET /api/session` without `Authorization`. An admitted network identity opens
the workspace with its exact principal label and viewer/operator role; a 401
leaves the bearer form available. Direct localhost access still requires an
admitted bearer. Manually entered tokens stay only in memory; clearing the
session erases private views, pending requests and download URLs. Reload and
back/forward restoration recheck network authorization and never restore a
cleared bearer token. There are no hosted accounts, persistent browser sessions
or permissive CORS. A browser clear-session action cannot revoke network access.

Export separate reviewed `config/web-access.ncl` and `config/web-automation.ncl`
inputs and pass their JSON outputs as `--access` and `--automation` to both
`check` and `serve`. They do not replace the original service/worker profiles.
Access policy admits independent viewer credentials and, optionally, exact
Tailscale user ID/login/role bindings with a distinct private proxy credential.
Viewer requests can inspect retained facts but cannot enroll, run, control or
configure automation. Worker credentials remain limited to exact worker ingress.
Denied writes are attributable audit observations, not canonical CI events.

For an operator-authorized private-tailnet deployment, preserve the original
backend origin, profiles, tokens and state. Bind Nginx only to the declared
Tailscale addresses; keep Funnel disabled and unrelated Tailscale Serve routes
unchanged. Admit the exact public Host and Origin, translate only that Origin
to the registered backend origin, and validate upstream TLS against its CA.
Missing-Origin writes still reach backend rejection; foreign origins are denied.

Use `kiln-web-service proxy-auth --access "$KILN_ACCESS_PROFILE"` behind Nginx
`auth_request`. This private loopback endpoint resolves the actual TCP peer
through the configured tailscaled Unix LocalAPI socket. Only registered exact
user ID/login pairs on untagged tailnet nodes are admitted. Nginx must overwrite
the peer socket and forwarded principal headers, clear client Authorization,
and supply the distinct proxy credential to the resolver and HTTPS backend.
Never trust browser-supplied identity headers or give every peer a shared
operator bearer. Unregistered/tagged peers fail closed before application effects.

Supply the proxy credential using systemd `LoadCredential` and a startup-generated
private Nginx include in a 0700 `RuntimeDirectory` under `UMask=0077`. Keep it out
of the Nix store, source, URLs, HTML/JavaScript and logs. Restart the proxy after
rotation. The local operator owns peer ACLs, principal enrollment, CA renewal,
credentials and listener exposure; Kiln dashboard/service maintainers own the
reusable identity and browser flow. This is not a hostile-code sandbox.

Enroll an allowlisted project, start its source-bound run and inspect exact
revision/tree, attempt, native execution, node state and canonical outcome. Finite
live snapshots reconnect with visible loss/uncertainty; they are not a complete
event history. Mutations use durable request identities and are never blindly
retried. Controls require current canonical revision, attempt, native execution
and sequence plus the selected capability; stale/unsupported controls fail before
effects and require fresh operator action.

stdout/stderr are actual retained Bounded Exec bytes, continuously uploaded
before command exit with **worker-owned byte cursors**. Several steps sharing a
native node append to its streams. Bounded Exec owns command termination while
private bounded transports retain output; cancellation/deadline stops the owned
process group and descendants. Ingress failure never masks the command outcome
or silently retries ambiguous uploads. Initial worker binding obeys one absolute
lookup deadline, including each HTTPS request and polling sleep.

Empty output, capture truncation, read bounds and retention are explicit; active
work need not produce new bytes yet. Diagnostic logs retain **native provider
cursors**. Log search operates on retained bytes, including chunk boundaries;
full-stream download supplies the exact retained bytes and BLAKE3 metadata.
Neither stream invents progress or completion. Text is inert; byte cursors are
never inferred from displayed UTF-8 character counts. Follow works with an
admitted network session as well as a manual bearer.

The web viewer reads one bounded chunk at a time. **Read from cursor** appends
when the requested cursor is the displayed stream/search's next cursor. Empty
reads preserve existing output and text selection. **Read from start**, or an
explicit different cursor, replaces the displayed range after a successful read.
Failed continuations and rejected seeks retain admitted text with an explicit
unavailable/stale status; they do not advance the accepted cursor. Changing a
run, node, stream or search invalidates old responses rather than mixing output.

Changing the literal search loads matching stdout/stderr from the beginning.
Diagnostic streams disable search and never submit `q`; switching back restores
the worker-stream search. Search skips are not source gaps. Follow scrolls only
while already at the tail, so scrolling up permits inspection of older text.
The existing display memory limit can still evict old visible text with a warning;
use an earlier cursor or the exact retained-stream download when needed.

Artifacts are real declared worker files, bound to exact source, attempt,
execution and known node. Uploads verify bounded bytes and BLAKE3 before atomic
owned retention; downloads use opaque IDs rather than caller paths. A receipt or
log is never substituted for a build product. Trusted worker declarations and
matching hashes do not prove product semantics or provider honesty.

The service has one state-directory owner, SQLite WAL/FULL durability and an
exact profile identity. Restart reconciles original admitted work; it does not
redispatch ambiguous work. Retention exhaustion rejects further intake rather
than silently pruning authority. Profile migration, retention rotation,
credential provisioning and worker isolation remain operator responsibilities.

#### Workspace navigation and recorded evidence

- **Overview:** retained totals, current project health, last-run completion time
  and recorded run-duration trends. Timestamps show browser-local date/time with
  its timezone alongside the exact ISO UTC instant. The same presentation is
  used in run and worker inspectors; locale/timezone do not change stored facts.
  Historical unknowns are excluded, not treated as zero. Missing/invalid times
  stay unavailable. Node timing is unknown when the first native observation is
  already terminal; no start time or duration is fabricated.
- **Workers:** configured execution targets, assigned projects, occupied/free
  project slots, queued work and exact latest execution observations. Metadata
  search, slot-state filters and independent exact selection are URL-restored;
  refresh and live snapshots retain selection. See [worker target inventory](#worker-target-inventory).
- **Runs:** metadata search, project/outcome/exact-source filters, oldest/newest
  ordering and stable bounded pagination. Cursors bind the filter/order and
  exclude later arrivals. A missing or filtered selection is not silently
  replaced. Page, run, node, inspection tab, comparison, appearance and density
  are URL-restored, including a full reload.
- **Pipeline:** admitted dependencies grouped by longest dependency depth, with
  recorded state badges/counts, measured node cards and highlighted immediate
  prerequisites/dependents. Stages do not imply execution order, readiness or
  placement. **Fit**, **100%**, and bounded 35–150% zoom affect only the scrollable
  diagram; long names wrap and the bounded full-node list remains available.
  Select a card or list entry for exact state/timing facts, relationship links
  and **View node logs**. Selection is URL-restored; same-run refresh preserves
  diagram zoom/pan, list position and keyboard focus. Missing, ambiguous or stale
  node selections are never replaced. Historical missing edges and invalid
  topology show an explicit warning instead of an invented or partial diagram.
  Unknown timing stays unknown; a recorded zero duration remains zero. Neither
  Completed nor a green badge grants CI, gate, review or release authority.
  Clearing the session clears all private workflow views. Kiln dashboard
  maintainers own this browser presentation; Lattice retains execution semantics.
  See [bounded rendering evidence](../.cairn/changes/improve-lattice-rendering/evidence/verification.json).
- **Rerun exact revision** creates a new source-bound attempt at the selected
  immutable revision. **Run latest admitted reference** resolves the current
  reviewed reference into a new candidate. **Retry known failed nodes** selects
  the failed/downstream subgraph and requires an exact registered provider route;
  missing subset support is rejection, never a full-graph fallback or success.
  Actions require explicit confirmation and fresh selected-run facts.
- **Tests & coverage:** reviewed worker steps may declare relative `reports`
  with `kind` `junit`, `cobertura`, `lcov`, `gate`, `review` or `provenance` and a
  `path`. Original profile bytes remain pinned; adding declarations requires a
  newly reviewed profile and exact provider route. Raw retained reports bind
  source, attempt, execution and node; malformed/stale/oversized uploads fail.
  Missing reports are unavailable, not passing tests or full coverage.
- **Compare & flaky:** compares retained outcome, duration, test status and
  coverage facts. Repeated-test instability requires actual pass/fail history
  for the same exact source revision, pipeline, node, report and test identity;
  a changed source alone never becomes a flaky-test claim.
- **Activity** distinguishes canonical events from attributable admission audit.
  **Evidence** exposes exact source/pipeline/execution/artifact linkage and
  recorded gate/review/provenance documents. Expand recorded-facts groups to
  inspect native values. A document's verdict is not accepted gate authority,
  review acceptance, source trust or release readiness.
- **Operations:** provider observations, durable queue order, admitted execution
  capacity and retained usage. A near-limit warning precedes intake exhaustion;
  it does not prune state, promise placement or convert unknown provider state
  into healthy capacity.

#### Worker target inventory

Authenticated viewers and operators can read `GET /api/workers`; worker-scoped
credentials cannot enumerate the control plane. The endpoint accepts no query
or body. Successful reads are immutable projections bounded by the existing
project/run limits; they do not contact providers or add persistence.

Targets group the exact provider-profile, workflow endpoint and platform binding.
Kiln-owned versioned BLAKE3 IDs remain stable across project ordering and public
label changes. Endpoint paths, repository paths, credentials, UID policy and
executable arguments never enter the response. The pinned Lattice adapter still
admits only `linux-x86_64`; this view grants no additional provider capability.

Each enrolled project supplies one admission slot, not a physical worker or CPU.
A nonterminal run outside the queue occupies its slot, including ambiguous
executions. A shared target with a free enrolled slot reports **Slots available**;
without free slots it reports **Unknown execution** for ambiguous work or **Busy**
otherwise. Unenrolled targets report **Not enrolled**. Physical liveness always
remains unknown. Failed durable storage returns unavailable (503), never usable
capacity. A queued or cancelled-before-dispatch request does not replace the
latest execution observation; missing recorded sequence or timing stays unknown.

The Workers inspector follows exact retained run IDs. List filters never replace
or clear the selected target, and a missing target remains missing. Clearing the
session erases private inventory and invalidates pending responses. Free slots
do not establish online status, provider placement or guaranteed dispatch.

Kiln web-service/dashboard maintainers own this view. Repeat its behavioral rail
with `cargo test --locked -p kiln-web-service --test engine workers::` in the
development shell. For runtime evidence, enroll an isolated project, observe a
real active and queued run, then terminal release; inspect Workers on desktop
and mobile, restore its URL, follow the exact run and clear the session.
Candidate-local observations are retained under
`.cairn/changes/add-dashboard-workers-view/evidence/`; they grant no deployment,
release or lifecycle acceptance authority.

#### Registered automation

Only server-reviewed registrations appear in **Automation**. Operators may
enable/disable declared sources/destinations, choose allowed push/PR categories,
adjust schedule intervals within reviewed bounds and select admitted notification
outcomes. Browser requests cannot add repository paths, credentials or URLs.

Signed push/PR intake verifies the exact repository/reference/current revision,
signature and replay identity before run reservation. Schedules reserve durably,
skip missed intervals rather than burst unbounded catch-up work, and preserve
their next due time across restart. A persisted round-robin selection gives due
schedules and pending notifications fair turns within `max_tick_effects`.

HTTPS completion/failure delivery reserves before sending and retains delivered,
rejected or ambiguous observations without blind resend. Bounded serial delivery
runs outside the engine mutex, keeping API/worker ingress available while a
receiver waits; the single poll executor resumes workflow/schedule polling after
the delivery batch. Configure notification timeouts and effect budgets with that
poll-delay bound in mind. Credentials/destinations never enter the page. No
declared trigger or notification means no invented subscription or destination;
the current operator owns enrollment and delivery policy.


### Internal developer loop

The service/worker maintainers own these optional contracts. Operators own their
reviewed profiles, credentials and host activation. Existing profiles omit the
new fields when unused; installing a new worker command still requires its exact
reviewed workflow operation, not an edit to retained identities.

The [corrected native developer-loop receipt](../.cairn/changes/complete-internal-developer-loop/evidence/native-07/verification.json)
records an actual four-node branching workflow, five measured steps, partial
failure, isolated cancellation/recovery, expiring worker observations, verified
cross-worker cache reuse, and committed/dirty CLI execution. The preceding
[viewer-browser receipt](../.cairn/changes/complete-internal-developer-loop/evidence/native-06/verification.json)
retains desktop (1440px) and mobile (390px) observations and screenshots. Both
trials retired all disposable processes, credentials and scratch. They used two
worker processes/work roots on one host, not two physical hosts. The corrected
trial's cold/warm build steps measured 744ms/870ms: reuse passed, but the warm
step was slower, so there is no performance-improvement claim.

The ownership-lifetime regression and full Cargo workspace pass after explicit
state-lock release. The Nix process fixture now uses non-core-dumping SIGKILL
and requires exactly that signal under its unchanged deadline; its focused
three-test rail passes. Earlier failures remain historical evidence, not current
passing receipts. The original notification failure did not recur under syscall
tracing; retained-description lock behavior was reproduced deterministically,
not inferred from a successful rerun.

For declared development coverage, explicitly select the optional
[`kiln-active-development` profile](../cairn-policy/development-coverage.ncl).
It includes the selected pending deltas and owned browser sources; it does not
change accepted-only release defaults or accept any change. Unknown anchors
still fail its negative control. Signed-population and installed-project links
point to the scoped retained observations above, not procedures marked as
implementation. See the [qualification results](../.cairn/changes/complete-internal-developer-loop/evidence/qualification.json).
No lifecycle acceptance, production activation or release-readiness claim follows.

#### Pipeline measurements and worker capacity

The run inspector shows admitted per-node step measurements and retained reports.
Elapsed duration uses the worker's monotonic clock. CPU usage measures the actual
command and its waited descendants; Linux peak RSS is the maximum child-process
peak, not aggregate concurrent memory or a cgroup reservation. Isolated commands
are accounted inside the namespace, outside the command's resource limits.
Cancellation or timeout can interrupt accounting; unavailable measurements remain
unknown. Historical runs do not acquire invented measurements.

Export `config/web-workers.ncl` and pass its JSON as `--workers` to both `check`
and `serve`. Each admitted worker has an exact principal, project, target and
maximum slot count. Derive the target before installing heartbeat authority:

```console
kiln-web-service worker-target --provider-profile "$PROVIDER_PROFILE_ID" \
  --socket "$LATTICE_SOCKET" --platform linux-x86_64
kiln-web-service heartbeat --profile "$WORKER_PROFILE" --identity "$WORKER_PROFILE_BLAKE3"
```

The worker profile's optional `heartbeat` config controls its stable worker ID,
target, interval, TTL and slots. Run one daemon per worker ID/private work root.
Independent workers may share one admitted project/target. Kernel-held job locks
and available CPU parallelism bound observed slots; process exit releases locks.
Heartbeats have authenticated sessions, strictly increasing observation sequences
and explicit expiry. Transport loss can skip observations without replacing the
session; policy rejection fails closed. Duplicate observations never renew TTL.
Offline workers contribute no observed available capacity. Configured project
slots remain separate from observed worker liveness and are not placement proof.

Optional worker `execution` selects the exact reviewed bubblewrap executable,
read-only toolchain mounts, network policy and per-command resource limits.
Preflight failure never falls back to account-authority execution. The command
gets its private writable checkout/temp and explicit environment, not service
credentials or the worker's repository backing store. This is bounded namespace
isolation, not a claim of hostile-code confinement or host-wide resource control.

#### Shared cache

Export `config/web-cache.ncl`; pass `--cache` to both `check` and `serve`. Its
private cache directory must not overlap canonical state. Policy bounds entries,
bytes, files, age and admitted project/producer/toolchain/platform/output paths.
Each cacheable worker step explicitly declares its inputs and output paths.
Compatible workers share keys based on those declarations and actual BLAKE3 input
hashes, not their local checkout, token or work-root paths. Producer/toolchain
identities must describe compatible commands and environments; a hit is not a
semantic correctness guarantee.

Restore verifies the entire admitted envelope and safe paths before writing.
Publication follows actual successful execution and admitted output ingress;
conflicting immutable content, corrupt bytes, links and credential paths fail
closed. Retention only prunes verified cache members, never journals, source or
run artifacts. The inspector reports actual miss/hit/publication, transferred
bytes and duration. Compare measured cold/warm runs; do not infer a speedup.

#### Developer CLI and explicitly selected dirty inputs

Export `config/web-developer.ncl` with an HTTPS origin, trusted CA, private token
file and finite request/watch bounds. Pin BLAKE3 of the original exported JSON
bytes, not a reserialized object. Submit an exact committed reference:

```console
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" \
  submit --project "$PROJECT" --request-id "$REQUEST_ID" \
  --reference refs/heads/main --revision "$BASE_REVISION"
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" watch --run "$RUN_ID"
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" logs --run "$RUN_ID" --follow
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" cancel --run "$RUN_ID" --request-id "$CANCEL_REQUEST_ID"
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" retry --run "$RUN_ID" --request-id "$RETRY_REQUEST_ID"
```

For dirty input, add `--dirty-repository "$WORKTREE"` plus repeated
`--include relative/file` and/or `--delete relative/file`. Only explicitly selected
changes are uploaded. The token must be outside that worktree. Unsafe, ignored,
linked or stale inputs are rejected. Submission binds a BLAKE3 manifest, exact
base revision and deterministic synthetic Git revision; it neither publishes the
source nor relabels the base commit as the executed tree. Independent backing
repositories reconstruct the same admitted bytes, without modifying the user's
index or worktree. Exact retry preserves snapshot provenance in a new run.

The selected Git worktree's `HEAD` must equal the supplied base revision during
capture. In a colocated Jujutsu fixture, create a worktree child of the admitted
bookmark (`jj new main`) before capture; the bookmark itself stays fixed.

Mutations are not blindly retried. Watch preserves success/failure/cancellation/
unknown outcome exits; logs retain real stream bytes and cursors. These commands
grant no source publication, release acceptance or remote mutation authority.

### Repeat the isolated native exercise

From the repository development shell, supply a compatible, exact measured
Lattice executable and matching source cohort with real shell-success semantics.
Ensure `bwrap`, `bash`, `jj`, `openssl`, `nickel`, `b3sum`, Python and user namespaces
are available. Scratch must be a new owned directory outside the producer's
private `/tmp`:

```console
cargo build --locked -p kiln-web-service
python3 scripts/web-service-smoke.py \
  --service "$KILN_SERVICE_EXECUTABLE" \
  --lattice "$LATTICE_EXECUTABLE" --lattice-source "$LATTICE_SOURCE" \
  --root "$KILN_NEW_EXERCISE_ROOT" --evidence "$KILN_EXERCISE_EVIDENCE"
```

`--browser-hold` additionally waits for actual browser observations in the owned
scratch `browser-done.json` after API checks and before resource retirement. The
native `passed` flag covers the API exercise; inspect `browser.passed` separately.
The rail creates a proper isolated CA and server leaf, disposable exact Git source, real reviewed commands/native
workflows and private credentials. It checks successful and failed execution,
multi-chunk output across repeated steps, empty streams, native diagnostics,
artifact bytes/hash, role/origin/TLS rejection, replay, restart, stale/unsupported
controls and canonical cancellation. Failed checks remain failures. It retains
measured executable/profile/source identities and observations, then removes its
processes, private keys, credentials and scratch tree.

The completion-notification restart check compares exact notification
idempotency identities at the real HTTPS receiver. An unrelated pending delivery
may finish during restart; total receiver traffic alone is not replay evidence.
Long activity identities wrap within record cards on narrow displays.

The complete-dashboard observations, including retained failures and corrected
deep-link/mobile browser checks, are under
`.cairn/changes/complete-live-dashboard/evidence/`. Their immediate consumer is
Kiln self-CI; dashboard/service maintainers own the reusable rail and operators
own host activation and separately admitted access/automation policies.

The separate [local activation receipt](../.cairn/changes/complete-live-dashboard/evidence/deployment.json)
records the supervised tailnet deployment, genuine tokenless operator identity,
log-follow cursor progression, byte/hash-verified retained-log download and
desktop/mobile observations. Original profiles, all four retained runs, eight
retained log members and unrelated tailnet routes were preserved. The listener
continues to use the original private deployment CA; clients must trust it.
The [live-response diagnostic](../.cairn/changes/complete-live-dashboard/evidence/live-completion-diagnostic.json)
retains Chromium cancellation events alongside completed finite snapshot and
heartbeat readback; it does not claim an empty browser diagnostics log.

The [verification record](../.cairn/changes/complete-live-dashboard/evidence/verification.json)
retains the broader Radicle mutation timeout failure and nine dangling
traceability IDs. Those failures remain release holds, not waived checks.

The isolated exercise proves only its recorded cohort, not host deployment.
Neither that exercise nor the local activation establishes hostile-code
confinement, source trust, arbitrary provider correctness, lifecycle acceptance
or release eligibility. Qualification does not transfer to changed executables
or profiles.

