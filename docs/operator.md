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
   nix flake check path:$PWD --no-write-lock-file -L
   ```

Run credential-isolated acquisition separately from a normal operator build: existing Cargo/Nix caches and credential helpers can conceal an inaccessible source input. Use a fresh temporary HOME/source cache with no Git/SSH credentials or global Git configuration, and acquire all direct and transitive sources before claiming the public quick start is reproducible. Consult `LICENSES.md` before combining adapters into a distributed executable.

For release preparation, bind actual command results to the exact validated source payload and retain failures. The original files under `evidence/` are historical publication evidence, not current check receipts. See `hosting-and-recovery.md` for the boundary between local preparation and separately authorized publication.


## Status

Status has three separate facets:

- readiness reports holds, delays, dependencies, and provider-local capacity;
- execution reports dispatch and provider observations;
- outcome reports the canonical attempt result.

Status is a rebuildable projection. It cannot mutate journal state.

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
