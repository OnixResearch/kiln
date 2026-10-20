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

Preparation discovers the actual home-activation application generated by the
consumer's inventory and Nickel `home-tools` service. It records and realises
the selected derivation's dependencies separately, **including generated
activation and home-file outputs**. Authenticated CI then realises the actual
application **launcher** through the no-substitution, no-remote-builder Nix
adapter. It does not freshly build the generated activation closure under CI,
execute activation or mutate a home. `provisioning.json` records that boundary.

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

The [retained qualification receipt](../.cairn/archive/1970-01-01-qualify-maintained-onixos-ci-pilot/evidence/verification.json)
binds the actual `Success`, `DomainFailure` and `ProviderError` cases to frozen
source, executable and input identities, with complete public command records
and an implementation-byte manifest. Its native repository readiness
conformance passes with `onix_release_ready = false`; this is not production
readiness or release approval.

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


