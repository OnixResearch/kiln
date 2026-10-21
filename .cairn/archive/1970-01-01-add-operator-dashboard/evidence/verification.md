# Operator dashboard verification

Date: 2026-10-21
Scope: a local read-only operator dashboard over one canonical Kiln journal
document, projected through the accepted `kiln-observe-core` projection.
Base revision: `30e3ff2` plus the uncommitted change working tree.

## What was added

- `crates/kiln-dashboard`: a pure rendering core over admitted observation
  views and an imperative shell that reads the named journal and profile.
- `config/dashboard-profile.ncl`: the typed dashboard profile, embedding the
  accepted observation profile.
- `fixtures/dashboard/profile-v1.json`: the exported profile fixture.
- `config/fixtures/dashboard-positive.ncl` and three negative dashboard
  fixtures.
- `docs/observation-dashboard.md`, the operator guide entry, and the README
  entries.
- A conformance audit that keeps the dashboard library free of filesystem,
  network, process, clock, and environment reach, and a registration of the
  crate in the reference-boundary audit.

## Results

- `cargo test -p kiln-dashboard`: 21 passed. The suite covers profile
  admission, a zero bound, a missing non-claim, the exported profile fixture,
  the state summary, cursor paging, page truncation, run and attempt selection,
  an absent selection, projection parity with the accepted core, redaction
  naming, the bounded event tail, canonical journal admission, a substituted
  event, an unsupported identity kind, a reordered record stream, a canonical
  prefix, the canonical JSON identity convention, a malformed document,
  deterministic rendering, the shell rendering
  the named journal, the shell rejecting a tampered journal, and watch mode
  redrawing the report.
- `cargo test --workspace`: pass.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass.
- `cargo fmt -- --check`: pass.
- `nix build path:$PWD#checks.x86_64-linux.nickel -L`: pass. The check
  formats and exports the dashboard profile, compares the checked-in JSON
  byte-for-byte, exports the positive fixture, and rejects every
  `negative-dashboard-*.ncl` fixture.
- `nix build .#checks.x86_64-linux.octet -L`: pass. That check aggregates the
  four Octet deny-all consumer checks for the runtime cohorts (aspen-runtime,
  kiln-aspen-host, kiln-radicle-nix-provider, and radicle-broker-wire). The
  Kiln main workspace, and therefore `kiln-dashboard`, is outside that scope:
  the workspace carries no `[workspace.metadata.octet]` configuration, and the
  `cargo-octet` 0.1.0 binary available in this environment is version-skewed
  with its own bootstrap source snapshot, so an ad hoc `cargo octet check` over
  the workspace cannot run here. The dashboard crate is covered by the
  workspace Clippy deny list, formatting, the test suite, the Nickel gate, and
  the Cairn gates.
- `cairn gate proposal|design|tasks add-operator-dashboard --root .`: PASS.
- `cairn validate --root .`: `valid: true`, with no findings, issues, spec
  issues, or substance findings.
- `cairn policy export --source cairn-policy/default.ncl --check`: fresh.
- `cairn traceability coverage --root .`: pass, 838 of 838 requirements
  referenced with no missing or dangling references after sync moved the delta
  requirements into `.cairn/specs/observation-dashboard/`.
- `cairn sync add-operator-dashboard --root . --execute`: applied the six
  requirements to `.cairn/specs/observation-dashboard/spec.md`.
- `cairn archive add-operator-dashboard --root . --execute`: archived to
  `.cairn/archive/1970-01-01-add-operator-dashboard/`.
- `cairn validate --root .` after archive: `valid: true`.

## End-to-end exercise

```console
$ cargo run -p kiln-dashboard --example sample_journal -- /tmp/kiln-sample-journal.json 4
$ cargo run -p kiln-dashboard -- --journal /tmp/kiln-sample-journal.json \
    --profile fixtures/dashboard/profile-v1.json --plain
kiln dashboard
  profile             dashboard-v1
  observation         observation-v1
  journal schema      kiln.sample.journal.v1
  declared identity   0000000000000000000000000000000000000000000000000000000000000000
  records             4
  state revision      4

summary
  runs                4   attempts 4   holds 0
  run outcomes        Pending 4
  attempt outcomes    Pending 4

runs
  page                pages 2   runs 4   requested 16   effective 2   capped
  run-0                   Pending   attempts 1
  run-1                   Pending   attempts 1
  run-2                   Pending   attempts 1
  run-3                   Pending   attempts 1

run detail
  run                 run-0
  outcome             Pending   attempts 1   active attempt attempt-0
    attempt-0               Pending   provider build-v1   products 0   metrics 0

attempt detail
  attempt             attempt-0
  run                 run-0
  outcome             Pending
  provider            build-v1
  external            -
  products            0
  metrics             0

events
  page                requested 32   effective 4   returned 4   capped   revisions 0..4
  0     run_started     run run-0  attempt attempt-0  provider build-v1
  1     run_started     run run-1  attempt attempt-1  provider build-v1
  2     run_started     run run-2  attempt attempt-2  provider build-v1
  3     run_started     run run-3  attempt attempt-3  provider build-v1

view identities
  state_summary       e51bcbef8b6916d0004738742913328fdcd24e75f509c5b1eda6c20c180127a9
  run_list[0]         25e2824bc396e9b51eb3dc9b9bee8afffe22efbbd3799db6e5546f3cab0ce0d0
  run_list[1]         d97d65bfc055171543b8fea9b1018b485edcd145056aa67bb9aadfd17a835474
  ...
```

The sample journal is a synthetic document produced by the crate's example, not
a live shell journal. The report shows the two bound effects the accepted
observation profile imposes: the run list is capped at the profile's page size
and paged with the accepted cursor, and the event tail is capped at the
profile's record bound.

## Limits and non-claims

- No live provider, remote repository, network connection, or production
  deployment was exercised. The dashboard opened no socket and issued no
  command or effect.
- The journal document's content identity is a writer-owned seal over a
  shell-specific tuple. The dashboard reports it as a declared fact and does
  not claim to have reproduced it.
- A journal that is a canonical prefix of a longer journal replays canonically
  and is admitted. The report states the revision it observed, and the
  dashboard does not claim journal completeness.
- The observation profile's admitted bounds cap the report. The shipped
  observation profile admits two run-list items per page and four event
  records, so the report is a bounded view of the journal rather than its
  whole history.
- An initial `nix build .#...` invocation against the Git-backed flake source
  reported an IO error while the working tree was uncommitted, because the
  Git-backed source snapshot excludes uncommitted files. The
  `path:$PWD#checks.x86_64-linux.nickel` rail includes the full working tree
  and passed. The Git-backed source rail must be rerun from a clean committed
  tree.
- No CI success, provider correctness, source trust, release readiness, or
  newest-revision claim is made.
