# Ten-minute demo script

Every command below was run on 2026-11-12 on one `x86_64-linux` host, inside
the development shell, on the launch tree before it was published, based on
source revision `428d03fcfb02478ed79d38453fb44f82c6a2bdd5`. Rerun all of them
on the tagged revision before presenting. The demo contacts no provider, uses
no credential and writes only under `/tmp`.

Say what each step shows and what it does not. Do not demonstrate a live
webhook, a real token or a hosted service: none is part of this script.

## Before you start

```console
nix develop path:$PWD
```

Run each step once beforehand so the audience does not wait for the compiler.
A cold build compiles the workspace from source.

`kiln --help` prints the command list and `kiln --version` prints
`kiln 0.1.0`. The other programs have no version flag; do not run one on
screen.

## 1. One command, one state (2 minutes)

```console
cargo run --locked -p onix-kiln-cli -- demo
```

Expected output, one line:

```json
{"revision":1,"runs":{"demo-run":{"active_attempt":"demo-attempt","outcome":"Pending"}},"attempts":{"demo-attempt":{"run_id":"demo-run","provider_profile":"standalone-build-v1","outcome":"Pending","external_reference":null,"products":[],"metrics":[]}},"holds":[],"seen_commands":["demo-start"]}
```

Say: one `StartRun` command was committed to an in-memory journal. The core
turned it into events and a requested effect, and the state is at revision 1.
The attempt is `Pending` because nothing dispatched the effect.

```text
state + command -> events + requested effects
state + event   -> new state
```

Do not say: a build ran. Nothing was executed.

## 2. Read-only observation (3 minutes)

```console
cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-preview
```

Expected output:

```text
Synthetic local preview; no provider was contacted.
Journal: /tmp/kiln-dashboard-preview/journal.json
Open: /tmp/kiln-dashboard-preview/dashboard.html
```

Open `dashboard.html` in a browser. The page is one self-contained file and
makes no network request. Point at:

- the page marks: "Read-only", "No provider effects." and "No CI control
  authority.";
- six runs and seven attempts, with run outcomes `Success` 2, `Pending` 1,
  `DomainFailure` 1, `Cancelled` 1 and `Unknown` 1;
- `api-checks`: a failed first attempt and a successful retry;
- `package`: outcome `Unknown`, shown as its own state rather than as a
  failure;
- the `kiln.observe.view.v1` identity under each detail view;
- the "Recent activity" table of journal events with their revisions.

Say: the dashboard admits a canonical journal through the same projection the
remote observation shells serve. It reads; it cannot control.

Do not say: these are real CI results. The journal is synthetic: "The sample
is developer rehearsal, not evidence of real CI execution"
([developer preview](../observation-dashboard.md#developer-preview)).

## 3. A forge loop on fixtures (3 minutes)

```console
cargo test --locked -p kiln-adapter-github --features protocol-fixtures --test local_ci_e2e
```

Expected result: `test result: ok. 17 passed; 0 failed`. Read out a few test
names, because they state the behavior:

- `passing_composition_executes_exact_source_and_publishes_success`
- `failing_command_publishes_domain_failure`
- `duplicate_delivery_reuses_run_without_second_job`
- `restart_recovers_original_execution_without_redispatch`
- `real_binary_rejects_invalid_signature_before_effects`
- `real_binary_denies_fork_pull_request`
- `real_binary_reconciles_ambiguous_status_without_repost`

Say: the packaged adapter binary runs against isolated source and status
fixtures. A signed delivery selects an exact commit, a job runs on that
source, and a status is published for that revision. A duplicate delivery
reuses the run. A restart recovers the original execution without a second
dispatch. An ambiguous status write is reconciled, not blindly reposted.

Do not say: this is a live GitHub run. "The provider rail uses isolated
source/status fixtures, not a new live GitHub run"
([hosted CI checklist](../hosted-ci.md#qualification-and-activation-checklist)).
The separate live exercise is one retained receipt for one recorded cohort;
cite it with its bound from [positioning](positioning.md#proof-points).

## 4. Where to go next (2 minutes)

Show, without running:

- the capability table in the [README](../../README.md#what-runs-today) and
  its evidence words;
- the [GitHub CI adapter procedure](../operator.md#github-ci-adapter) for a
  real repository: webhook secret, repository-scoped token, reviewed Lattice
  workflow;
- the [claim boundary](../../README.md#claim-boundary).

Close with the three limits: Kiln is not a sandbox, no production
qualification is claimed, and hosted CI is planned and not available.

## Questions to expect

Answer from [positioning](positioning.md#objection-handling). If a question
has no supported answer there, say that the repository does not establish it.

## Recording

Record the terminal and the browser page only. Do not simulate a hosted
console, a customer account or a billing page. Keep revision and date visible
in the recording description.
