# Attempt outcome reporting verification

Issue: <https://github.com/OnixResearch/kiln/issues/90>.

## Delivery and ownership

The immediate result is an opt-in `kiln-implementer metrics-report` over the
existing durable attempt ledger. The reusable capability is pure deterministic
outcome/effort aggregation with exact sample denominators, incomplete telemetry
coverage and immutable exact-attempt observations. Kiln maintainers own the
reporting core, SQLite consumer and operator command. Lattice owns worker usage
measurement; this change does not add a worker runtime or billing service.

## Exercised checks

Commands ran from the Kiln checkout. Dependency-bearing CLI/web checks used
`nix develop path:$PWD -c`; Rust was 1.88.0.

| Check | Observed result |
| --- | --- |
| `cairn gate proposal report-attempt-outcomes --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"` | PASS before implementation |
| Equivalent `gate design` and `gate tasks` | Both PASS before implementation |
| `cargo test --locked -p onix-kiln-core implementation::metrics` | 6 passed |
| `cargo test --locked -p onix-kiln-core implementation::attempt_report` | 11 passed |
| `cargo test --locked -p kiln-implementer --test durable` | 17 passed |
| `cargo test --locked -p kiln-implementer --test durable metrics_report_never_writes_state_versions_or_legacy_rows` after fixture field-access correction | 1 passed; state versions and bytes unchanged |
| `cargo test --locked -p kiln-adapter-lattice worker` | 3 passed |
| `cargo test --locked -p kiln-adapter-github --test implementer_cli` | 1 passed; actual binary usage input, report and invalid-window paths |
| `cargo test --locked -p kiln-web-service --lib engine::verification` | 6 passed; environment-specific new test ignored in this invocation and executed separately below |
| `cargo test --locked -p kiln-web-service --lib completed_worker_keeps_real_controller_outcomes_separate_after_restart -- --ignored --nocapture` with the explicit private Nix cohort | 1 passed; actual pass, rejection and timeout/unknown check execution, then SQLite reopen |
| `cargo check --locked -p onix-kiln-core --target wasm32-unknown-unknown` | PASS; no-std target |
| `cargo clippy --locked -p onix-kiln-core -p kiln-implementer -p kiln-adapter-lattice -p kiln-adapter-github -p kiln-web-service --all-targets -- -D warnings` | PASS |
| Native `cairn sync report-attempt-outcomes ... --execute` | One additive requirement merged into `.cairn/specs/implementation-work/spec.md`; no archive |

The real verifier fixture used a temporary private root under the operator's
home, removed afterward. Its immutable executables were:

- Git: `/nix/store/xgw813vgzl2q0696sq45adpj0xd690jq-git-2.56.0/bin/git`.
- Shell: `/nix/store/kwyi4bq5s2qgwbhz6cs4gjy57c2zdxws-bash-interactive-5.3p20/bin/bash`.

`KILN_VERIFICATION_TEST_ROOT`, `KILN_VERIFICATION_TEST_GIT` and
`KILN_VERIFICATION_TEST_SHELL` select that explicit environment. The fixture
retains the same Completed worker observation while independently executing a
successful check, failed check and deadline-expired check. Only the first gets
an accepted verification receipt; nonpassing phases are not advanced by metrics.

## Actual CLI smoke over synthetic retained history

A throwaway Python scenario seeded an isolated SQLite database with six clearly
synthetic finished attempts, invoked the real `kiln-implementer metrics-report`
through `cargo run --locked -p kiln-adapter-github --bin kiln-implementer`, and
checked the JSON and unchanged retained row/version. It did not admit those
synthetic receipts as candidate evidence.

The `[100, 101)` claim-time window produced one verified pass, five nonpasses,
Completed/rejected shares of 1/5 and 1/2, and one member in each failure category.
It retained 28 complete observed turns and one missing turn sample, leaving the
full-window turns/pass ratio null. Claim-to-end effort was 210 milliseconds per
verified pass. Three attribution groups included explicit unknown metadata.
The temporary database, enrollment file and script state were removed.

## Boundaries and encountered failures

- Initial plain-Cargo CLI compilation lacked `pkg-config`/OpenSSL. The documented
  Nix development shell supplied them; the CLI and web checks then built.
- New fixture compilation, unchecked JSON field access and fixture enrollment
  mismatches were corrected before the passing checks. Temporary Git failure
  diagnostics were removed. No production verifier policy was weakened.
- The host's kache index reported an I/O error and disabled caching. Compilation
  and the listed checks still succeeded without cache hits/stores.
- `cairn validate` passed before the accepted-spec sync. Repository-wide
  `cairn traceability coverage` failed on unrelated dangling references and the
  missing `kiln.github.landing.merge.scenario.landed` and
  `kiln.github.source_status.scenario.close` evidence anchors. It is not a passing
  repository-wide coverage or release-readiness claim.
- Current `lattice.omp-jsonl-observation.v1` terminal observations are consumed,
  but model requests remain `Absent/NotInstrumented`. JSONL event counts,
  configured limits and reserved charges are never measured turns. Missing
  normalized terminals are not reconstructed from timeout/error prose.
- No dollars, profile ranking, model-weight attestation, live OMP/model campaign,
  remote landing or lifecycle archive was exercised. Full workspace tests,
  Octet, release-readiness and flake checks were not run. No Nickel configuration
  changed. Subsequent operator-authorized publication/deployment is recorded below.

## Final documentation checks

- Scoped `rustfmt --check --edition 2024 --config skip_children=true` passed
  for all 16 changed Rust files.
- Final `cairn validate --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"`
  passed after spec sync and completed task recording: no issues.
- Final traceability coverage still failed with 71 unrelated dangling IDs and
  the two missing landing/source-status anchors named above. None of the
  `kiln.implementation.outcome_metrics` requirement/scenario IDs was dangling.
  These unrelated gaps were left unchanged; no passing whole-repository
  coverage receipt is claimed.

## Authorized publication and operated deployment

After the user requested commit, push and deployment, source commit
`9e9c91ed62670977d046711456a34cfd4b4ba1f8` was published to canonical Radicle
and its GitHub projection. Both exact refs were read back; the existing native
Radicle signed-reference route reported synchronization with two seeds. The
original checkout's helper failure was bypassed with an isolated exact-source
Jujutsu clone, not by changing signing or governance. Delegate, threshold,
signed-ref and source NAR BLAKE3 observations are in
[`deployment.json`](deployment.json).

The exact committed-source Nix web-service package built successfully, passed
preflight with all eight unchanged profile arguments, and replaced only that
installed package. The running executable resolved to the built store path.
CA-verified HTTPS health passed locally and over the private tailnet; all 19
retained runs, 116 configuration files, unrelated installed profile elements
and the Lattice PID were preserved. Fourteen served script bodies matched the
frozen source bytes. Private consistent SQLite/configuration rollback material
and both package roots were retained outside the repository.

Chromium refused the private CA. No certificate exception was applied, and no
rendered operated-browser verification is claimed. These observations validate
this one activation, not production qualification, fresh remote acquisition,
full repository readiness or later branch tips.
