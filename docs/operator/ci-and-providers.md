# CI execution, local checks, and provider adapters

This runbook covers the local reference verification rail, isolated Patchgate review, Nix job discovery, the Aspen native runtime, and source/workflow/forge adapters (GitHub, Gitea/Forgejo, Iroh, and Radicle).

> **Adapter tier note:** The live Factory workspace `default-members` tier includes Git, Jujutsu, GitHub, Radicle, Lattice, Nix, Cairn, Patchgate, Mantle, Aspen, and Iroh. Four experimental/off-by-default adapters (`kiln-adapter-slurm`, `kiln-adapter-tangled`, `kiln-adapter-pijul`, and `kiln-adapter-gitea`) are excluded from `Cargo.toml` `default-members` and built or tested explicitly via `-p <crate>` or `--workspace`.

For the operator runbook index, see [the operator guide](../operator.md).

## Local reference run

1. Enter the development shell:

   ```console
   nix develop path:$PWD
   ```

2. Run the standalone fixture:

   ```console
   cargo run -p onix-kiln-cli -- demo
   ```

3. Run all checks:

   ```console
   cargo test --locked --workspace
   cargo clippy --locked --workspace --all-targets -- -D warnings
   cargo fmt --all --check
   scripts/check-nickel-fixtures.sh
   cairn validate --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"
   cairn traceability coverage --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json" --json
   cairn release-readiness --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"
   nix build path:$PWD#checks.x86_64-linux.octet --no-link -L
   nix flake check path:$PWD --no-write-lock-file -L
   ```

   `nix develop path:$PWD` provides Kiln's pinned `cairn` wrapper defaulting to
   `cairn-policy/generated/cairn-policy.json` (`kiln-default`) regardless of
   outer `PATH` or `CAIRN_POLICY`. Pass `--policy` explicitly on policy-bearing
   commands (including `sync` and `archive`) and record the resolved binary,
   `policy`, and `policy_hash` in retained evidence.
   `scripts/check-nickel-fixtures.sh` exports every positive fixture and
   asserts every negative fixture is rejected; the same script runs inside
   `checks.x86_64-linux.nickel`.
   `cairn release-readiness` consumes two checked-in gate fixtures.
   `fixtures/agent-completion/` binds the maintained repository gate commands
   to the current generated policy hash; regenerate its command and selection
   hashes whenever those commands or the policy change.
   `fixtures/preserves-canonicalization/corpus.json` binds canonical Preserves
   values to their BLAKE3 hashes; run
   `cairn preserves-corpus update --path fixtures/preserves-canonicalization/corpus.json --execute`
   after adding or changing a case.

<!-- r[verify kiln.toolchain.pin.scenario.shell] -->
<!-- r[verify kiln.toolchain.pin.scenario.older] -->
   The development shell and every root check use the compiler that
   `rust-toolchain.toml` pins (Rust 1.97.1), and the workspace `rust-version`
   declares the same release. The [toolchain receipt](../../.cairn/archive/2026-11-22-raise-rust-toolchain/evidence/verification.json)
   retains `nix develop -c rustc --version` reporting 1.97.1, and Cargo refusing
   the root crates under Octet's older rustc 1.96 nightly until its overlay
   checks passed `--ignore-rust-version`. That is the tested compiler, not a
   proven minimum.

Run credential-isolated acquisition separately from a normal operator build: existing Cargo/Nix caches and credential helpers can conceal an inaccessible source input. Use a fresh temporary HOME/source cache with no Git/SSH credentials or global Git configuration, and acquire all direct and transitive sources before claiming the public quick start is reproducible. Consult `LICENSES.md` before combining adapters into a distributed executable.

For release preparation, bind actual command results to the exact validated source payload and retain failures. The original files under `evidence/` are historical publication evidence, not current check receipts. See `hosting-and-recovery.md` for the boundary between local preparation and separately authorized publication.



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
kiln-patchgate-review run --profile profile.json --intent intent.json --output receipt.json
kiln-patchgate-review reconcile --profile profile.json --plan plan.json --receipt receipt.json > reconciled.json
```

`run` admits the intent, reviews it, and publishes exactly one
`kiln.patchgate.review.v1` receipt by staging a temporary file in the destination
directory, syncing it, renaming it into place, and syncing the directory. It
rejects a symlink or non-regular destination and prints only a bounded summary
(with the receipt path and decisions), not the private native report. A reviewed
worker step can invoke `run` with `--output patchgate-review.json` and declare
`reports = [{ kind = "review", path = "patchgate-review.json" }]`; the worker
then uploads that exact receipt as run-bound review evidence. This retention
path does not grant review, merge, gate or release authority.

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

<!-- r[impl kiln.dashboard.reviews.view] -->
The **Patchgate receipts** page (3.2 in the console's numbering) and each run's
**Reviews** tab show a receipt the same way, from the human facts to the exact
ones:

1. A headline with the first change's message, the receipt file and the latest
   decision time, and a tally of decisions with what needs a person first.
2. A plain statement that the receipt is evidence only. It warns when the
   producer claims acceptance authority, when isolation is not recorded, or
   when Patchgate's report cannot be read.
3. One entry per change: its decision and what that decision means, Patchgate's
   next step, and the change's message, paths, decider (Patchgate's rules, a
   model or a person), decision time, reason and prerequisites. Below these
   come the full change hash with a copy control, and any model, context
   coverage, Nix check or secret-finding evidence.
4. The review context: the main and inbox channel states, the trusted
   configuration and policy, protected and documentation paths, model requests
   and review time, and the Patchgate and Pijul versions.
5. The exact identities: the receipt's BLAKE3 and size, plus the request,
   report, effect, idempotency, candidate, candidate-root, pipeline-revision,
   policy, provider-profile and native-binding identities, and the producer's
   `isolated` and `authorizes_acceptance` claims.
6. Patchgate's report verbatim, then the complete receipt.

Each decision comes from Kiln's admitted observation. Messages, paths,
deciders, reasons and times are read from Patchgate's native report and are
labeled as recorded by Patchgate. A report that is not readable JSON leaves
every decision in place and marks that context unavailable.

All nine decisions read in their own words. The meanings follow Patchgate's
state model:

| Decision | Meaning | Next in Patchgate |
| --- | --- | --- |
| Accepted | Patchgate accepted the change | — |
| Rejected | It stays in the inbox and is not applied to main | Remove or replace it, then record a corrected change |
| Escalated | A person must decide (protected path, unmerged conflict, oversized change, unsettled review) | Resolve it in a separate workspace, or reject it |
| Blocked | A prerequisite was rejected, escalated, removed, blocked or is unknown | Decide the prerequisites first |
| Waiting | A prerequisite is not in main yet | Decide the prerequisites first |
| Pending | Not decided yet | Review the change |
| Removed | Accepted before, no longer in main | Examine the removal before deciding again |
| Reverted | Accepted, then removed with `patchgate revert` | Examine the removal before deciding again |
| Unknown | A decision record Patchgate does not recognize | Examine the channel and the prerequisites |

Any other value reads as "Unrecognized" with the recorded text, and a missing
one reads as "Not recorded". Neither reads as accepted. None of these
decisions is folded into "passed".

The page lists receipts newest decision first. Each is named by its first
change's message and shows a tally, the number of changes, the decision time,
and a flag when the producer claims authority or isolation is not recorded.
Search narrows the list by file name, message, change hash or the candidate,
effect or report identity. The decision filter, including **Needs attention**
for Patchgate's stuck states, narrows it as you type. Both filters are kept in
the URL (`receipt_q`, `receipt_decision`). A selected receipt that the filters
hide stays selected. The arrow keys move between receipts. With nothing
selected, the right-hand panel explains every decision.

In a run's **Reviews** tab, each receipt also shows when it was retained, its
review node, source revision and report record, and offers **Download exact
receipt** for the retained bytes. A review report that the run declared but
that is missing or unreadable is shown as unavailable. Other reports stay under
**Tests & coverage**. Both surfaces are read-only, never dispatch Patchgate and
grant no review, merge, gate or release authority. Upload the receipt JSON as a
`review` report to retain it for a run.

### Retained receipt directory

The workspace's **Reviews** page lists `kiln.patchgate.review.v1` receipts from
one explicitly admitted directory; it never scans elsewhere. Apply
[`config/web-reviews.ncl`](../../config/web-reviews.ncl) to an operator-reviewed
`{ label, directory }` record and pass the exported JSON with `--reviews` on
both `check` and `serve`.

```console
nickel typecheck config/web-reviews.ncl
nickel export --format json --output "$KILN_REVIEWS_PROFILE" "$KILN_REVIEWS_NCL"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE" --reviews "$KILN_REVIEWS_PROFILE"
```

`/api/reviews` returns the label, plus each receipt's name, byte count, BLAKE3,
effect, candidate and report identities, per-decision counts, normalized
decisions, and the producer's `isolated` and `authorizes_acceptance` claims.
For listing, it adds two facts read from Patchgate's native report:
`first_change_message` (the first line of the first normalized change's
recorded message, at most 160 characters) and `latest_decision_at_ms` (the
latest RFC 3339 decision time among the native records). Both are null when the
report cannot be read. `/api/reviews/NAME` returns the verbatim document.
Non-receipt entries, symlinks, oversized files and documents with another
schema are listed under `skipped` with a reason, never as empty success. The
directory is server-owned; names are bounded and traversal is rejected. Readers
are authenticated viewers/operators; workers are denied.

### Produce and retain one receipt

#### Outside a run

`scripts/patchgate-review-job.py` builds the profile and intent from live native
states, computes one archive identity per pending inbox change, and invokes
`kiln-patchgate-review run`, which admits the intent, reviews it and publishes
one receipt atomically:

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

#### Run-bound worker step

`config/patchgate-review-job.ncl` exports one typed `Step` for
`config/web-worker.ncl`. Include it in the reviewed worker profile `steps`
array with `node_id` equal to the admitted pipeline's review node:

```ncl
(import "config/patchgate-review-job.ncl") {
  node_id = "review",
  python = "/nix/store/.../bin/python3",
  script = "scripts/patchgate-review-job.py",
  target = "/srv/kiln/patchgate-review-target",
  adapter = "/nix/store/.../bin/kiln-patchgate-review",
  patchgate = "/nix/store/.../bin/patchgate",
  pijul = "/nix/store/.../bin/pijul",
  home = "/var/lib/kiln/patchgate-review-home",
  pipeline = "patchgate-review-v1",
  output = "patchgate-review.json",
  path = "/run/current-system/sw/bin",
}
```

The checkout supplies the script; `target` supplies `patchgate.json`, its
policy, the native channels and the pending inbox. The target is an
operator-provisioned local Pijul repository and need not be the Git checkout.
Set `execution.read_only_data_mounts` to the target's canonical directory so a
sandboxed worker can read it. By default mounts are limited to `/srv` and
`/var/lib/kiln`; a reviewed profile may set
`execution.read_only_data_mount_roots` to extend the roots within the reviewed
`/srv`, `/home`, `/mnt`, `/opt`, and `/var/lib/kiln` families. Mounts must be
canonical directories with no symlink resolution and are bound read-only. The
target must be inside the declared mount; the checkout, receipt output and
`/tmp` review home remain the only writable locations. A missing or foreign
target fails closed without account-authority fallback.
`path` must resolve `b3sum`; the script passes `HOME` and `PATH` explicitly to
Patchgate and Pijul. The step declares the receipt as a `review` report, so
worker report upload binds the exact run, attempt, execution and node. A missing
receipt is unavailable, not a clean review. The step grants no review, merge,
gate or release authority.

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
`.cairn/archive/1970-01-01-add-patchgate-review-adapter/evidence/native-smoke.json`.
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
different existing bytes. See the [immutable source receipt](../../runtime/onix-eval-cohort/jobset.source.json)
and [exact runtime paths, hashes and manifest contract](../nix-eval-jobs-adapter.md#frozen-source-and-measured-runtime).
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
[integration qualification receipt](../../.cairn/archive/1970-01-01-add-onix-nix-evaluator-cohort/evidence/verification.json)
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
or release authority. Review [license provenance](../../LICENSES.md#optional-onix-discovery-provenance)
before distributing the fixture or runtime cohort; wrapper `NOASSERTION` is not
a permissive license grant.


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
[`runtime/kiln-aspen-host/README.md`](../../runtime/kiln-aspen-host/README.md).
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
`opened`, `reopened`, and `synchronize` can start jobs. This standalone service
has no approval surface, so it admits only `untrusted_pull_requests = "deny"`:
fork pull requests are refused (`github_fork_denied`). The live web service's
[untrusted pull request policy](deploy-and-upgrade.md#untrusted-pull-requests) can instead hold them
for operator approval. A signature does not establish delivery freshness, source
trust, or safety of executing repository contents.

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

<!-- r[impl kiln.lattice.runtime_pin] -->
Use Lattice runtime `85a38b6a4bceeae4bb2057cf157e21752100f1b5`, separately
from Kiln's MIT contract/transport import `70496e67c7fd4a8b05914161a8e09de2759bebc8`.
It includes `shell_success` (`6339864d3c4e5de72ba4f0899b11b5d5b76704b6`) and
observed cancellation (`813076d7e42d0ddfb65042bee615bc5f12a2b4d3`). Producer requirement
`lattice.workflow_interchange.shell_success` requires that exchange shell commands exit
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
   [the workflow contract](../lattice-workflow.md), request schema
   `lattice.workflow-request.v2`.
4. The helper acquires only that exact commit via GitHub HTTPS, with no SSH
   transport or ambient credential fallback. It validates the bounded snapshot,
   rejects symlinks, gitlinks, and `.git` path components, materializes a fresh
   temporary checkout without Git metadata, and executes reviewed argv there.
   Acquisition, transfer, snapshot, command, output, and teardown bounds remain
   finite; credentials and inherited environment are not forwarded to the
   command. Bounds and temporary directories are **not a sandbox**. Run jobs
   under a separate worker identity and operator-provided host/container/VM
   isolation. Even same-repository PR code may be hostile. Fork and
   outside-contributor runs need the live web service's reviewed `approve`
   policy and an untrusted-capable runner, not a changed fixture.

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

### GitHub issue label and `/runbook <id>` comment routing

`config/implementer-policy.ncl` accepts an optional `runbook_routing` record
binding exact GitHub issue labels and exact `/runbook <id>` comment commands
to reviewed runbook IDs, fixed target references, and fixed parameters (for
either single dispatch via `POST /api/runbooks/{id}/dispatch` or fan-out via
`POST /api/runbooks/{id}/fanout`). Issue and comment prose never supplies
commands, runbook IDs outside the reviewed mapping, parameters, targets, or
budgets.

- **Separate webhook queues**: `config/github-ci.ncl` accepts an optional
  `runbook_routing_queue` distinct from `implementation_queue` (`impl_q.path !=
  rb_q.path` enforced at admission). When configured, `issues` webhooks enqueue
  to both queues and `issue_comment` webhooks enqueue only to
  `runbook_routing_queue`. Operators running `kiln-implementer drain` and
  `kiln-implementer drain-runbooks` must supply separate `--queue` paths.
- **Security and RBAC**: Deliveries (`issues` `labeled` on open issues, and
  `issue_comment` `created` on issues or PRs) require valid HMAC-SHA256
  authentication, enrolled repository match (`id`, `owner`, `name`, plus
  `Enrollment.repositories` check), non-bot actor membership in both
  `Enrollment.actors` and `runbook_routing.allowed_actors`, and push-access
  `author_association` (`OWNER`, `MEMBER`, or `COLLABORATOR`). For `issues`
  `labeled` events (and `poll-runbooks` label polling, which reads GitHub's
  `/issues` endpoint), both the labeler/issue-creator actor and
  `issue.author_association` must satisfy push access; labels added to
  external-contributor issues are refused (`github_outside_contributor_denied`),
  so maintainers should trigger runbooks on external issues via
  `/runbook <id>` comments instead. Pull request comments additionally verify
  `head.repo` via `GET /repos/{owner}/{repo}/pulls/{number}` and fail closed on
  forks (`github_fork_denied`) or unverified origins. Every refused event is
  recorded in `RunbookRoutingStore` without secrets or raw prose.
- **Idempotency and reply comments**: `kiln-implementer drain-runbooks` and
  `kiln-implementer poll-runbooks` deduplicate on both `X-GitHub-Delivery`
  (`body_identity` verified) and canonical source key (comment ID or
  issue/label), derive deterministic `gh-rb-<blake3>` `request_id`
  values, and post a concise issue/PR reply comment with only the runbook ID,
  run/fan-out ID, optional `console_base_url` link, and hidden reconciliation
  marker (never credentials, tokens, `LATTICE_PARAM_*` values, local paths, or
  raw logs). Both CLI subcommands are registered as `OperatorConfig` in
  `ENTRY_POINTS`.

The retained [local runtime evidence](../../evidence/github-ci-local-runtime.ncl)
imports the [raw observations](../../evidence/github-ci-local-runtime.json) and binds
the exercised executables and selected producer files by BLAKE3. It covers exact
source success/failure, restart duplicate suppression, and lost-status-response
reconciliation against local fixtures. It also records scoped Lattice-wide gate
results; it does not bind a clean producer release or prove deployment.

## Gitea and Forgejo CI adapter

The standalone Gitea/Forgejo shell uses `config/gitea-ci.ncl` and
`config/gitea-job.ncl`. It is separate from the hosted connection composition
and does not grant repository, merge, or release authority.

1. Create a repository-scoped token with contents read and commit-status
   read/write. Keep it in a SecretSpec reference distinct from the webhook
   secret.
2. Export the service profile with the numeric repository id, normalized
   `status.api_base` ending in `/api/v1`, replay path, SecretSpec references,
   and reviewed workflow profile.
3. Create a Gitea or Forgejo JSON webhook to the externally reachable
   `/gitea/webhook` URL with push and pull-request events and the same secret.
4. Start `kiln-adapter-gitea --profile PROFILE.json`, inspect runs with
   `status`, and recover ambiguous effects with `reconcile`.
5. Export the job profile with the normalized `git_base`, reviewed argv,
   environment, snapshot/transfer/deadline bounds, and optional source
   SecretSpec reference. `kiln-gitea-job` verifies the BLAKE3 of the exact
   profile bytes before acquisition.

The listener is loopback-only; terminate TLS in front of it and forward the
original body and header bytes. The adapter accepts `X-Gitea-*`,
`X-Forgejo-*`, and GitHub-compatible headers, rejects duplicate/conflicting
aliases, and admits only 40-hex SHA-1 revisions and JSON deliveries. Fork pull
requests are denied. Status writes and reads use the Gitea commit-status API;
ambiguous writes remain unknown and are reconciled read-only.

See [the Gitea/Forgejo procedure](../gitea-ci.md) for the exact profile fields,
commands, failure behavior, and non-claims.

## Iroh remote workflow execution

`kiln-adapter-iroh` carries the existing Lattice workflow exchange between a
controller-local Unix proxy and a worker-local Lattice socket:

`Kiln workflow port -> private Unix proxy -> authenticated Iroh -> worker -> Lattice`.

It adds transport, not another executor. Dispatch, exact recovery, observation,
controls, logs and reconciliation keep their native identities and existing Kiln
admission. The separate read-only Iroh observation plan is not implemented here.
The registry cohort pins Iroh `1.1.0` under ALPN `kiln.workflow.v1` and builds
with the workspace's pinned Rust toolchain.

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

The [operated proof](../../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json)
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
  `refs/heads/...` branch or `refs/tags/...` tag, source profile ID and snapshot
  bounds;
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
compatibility](../radicle-status.md). Tangled/Radicle job objects use
the separate [job publication](../radicle-jobs.md) port. Repository Atom
feeds and external feed subscriptions are configured through the
[repository feeds](../repository-feeds.md) policy. No command implicitly
synchronizes peers.

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

The [retained qualification](../../.cairn/archive/1970-01-01-compose-radicle-source-to-build-ci/evidence/verification.json)
links the exact native cohort, original commands, canonical journals, source and
build observations, rejected attempts, root release and retirement records.

### Native Radicle peer and broker canary

The [live qualification](../../.cairn/archive/1970-01-01-qualify-live-radicle-ci/evidence/verification.json)
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

The [distinct-host qualification](../../.cairn/archive/1970-01-01-qualify-distinct-host-radicle-ci/evidence/verification.json)
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

The [status qualification](../../.cairn/archive/1970-01-01-qualify-distinct-host-radicle-status/evidence/verification.json)
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

