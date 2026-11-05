# Cancel-only Lattice cohort verification

Observed on 2026-11-05 with pinned Rust 1.88.0 and Nickel 1.17.0.

- `cargo test --locked --offline -p kiln-adapter-lattice`: **21 passed**, including concrete Unix socket exchange coverage, unsupported pause/resume before transport, and unsupported plan controls before dispatch. An unused constant left by removal of the old pause-success test was subsequently removed.
- Nickel exports accepted `lattice-workflow-positive.ncl` and `web-service-positive.ncl`; rejected `negative-lattice-workflow-control.ncl`, `web-service-pause-negative.ncl`, and `web-service-resume-negative.ncl`.
- Actual `kiln-web-service reviewed-request --profile <exported-positive-profile> --project fixture` emitted controls `{cancel:true,pause:false,resume:false}`.
- Bypassing Nickel with JSON profiles setting pause or resume to true caused the actual CLI to fail with `unsupported reviewed workflow` for each control.

The maintained profile requires Dispatch, Observe, Cancel, Reconcile, and ReadLogs. Generic Paused observation mapping and the unmodified historical MIT producer fixtures remain available independently; this cohort does not promise pause/resume execution. External runtime deployment and independent historical Aspen-host execution are not established by these checks.

Consumer checks also passed with the updated cohort: `cargo test --locked --offline -p kiln-adapter-github -p kiln-adapter-gitea -p kiln-adapter-iroh` (GitHub 52 unit tests, Gitea 54 unit tests, Iroh 1 binary test plus 8 workflow integration tests; associated doctests passed). Cargo warned about the existing duplicate `live_ci_inputs` example output names across the two forge adapters; these were not renamed as part of a capability-cohort change.

Independent read-only review of the #37/#38 source/configuration/documentation boundary reported no actionable correctness findings.
Final integration also exercised the complete Nix Nickel check, including exact
typed preboot-policy/export equality and all positive/negative configuration
exports. That check exposed an unbound `Exact` predicate in the Radicle workflow
consumer; the binding now follows the existing Iroh convention. Forced pause
and resume overrides have dedicated negative Radicle fixtures, and the complete
configuration check passed after the fix.

The full GitHub protocol-fixtures suite also passed: **78 tests** across seven
summaries, both locally and in the Nix fixture gate. Its stale direct access to
private admitted-config fields was migrated to the existing typed accessors;
the production admission boundary was not weakened.

