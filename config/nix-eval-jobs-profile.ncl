# r[impl kiln.nix_eval_jobs.cohort_admission]
# r[impl kiln.nix_eval_jobs.input_admission]
# r[impl kiln.nix_eval_jobs.onix_boundary]
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let AbsolutePath = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^/" value
) in
let Blake3 = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Policy = {
  flake_mode | Exact true,
  expression_mode | Exact false,
  allow_apply | Exact false,
  allow_impure | Exact false,
  allow_force_recurse | Exact false,
  allow_registries | Exact false,
  retain_gc_roots | Exact true,
  partial_admission | Exact false,
  check_cache_status | Exact true,
  include_metadata | Exact true,
  include_input_derivations | Exact true,
} in
let Limits = {
  max_line_bytes | Positive,
  max_total_stdout_bytes | Positive,
  max_stderr_bytes | Positive,
  max_records | Positive,
  max_jobs | Positive,
  max_job_errors | Positive,
  max_metadata_bytes | Positive,
  max_diagnostic_bytes | Positive,
  max_outputs_per_job | Positive,
  max_dependencies_per_job | Positive,
  max_tasks | Positive,
  max_gc_roots | Positive,
  max_executable_bytes | Positive,
  max_manifest_bytes | Positive,
  max_in_flight | Positive,
  max_workers | Positive,
  max_memory_mebibytes | Positive,
  stdin_max_bytes | Positive,
  timeout_milliseconds | Positive,
  poll_interval_milliseconds | Positive,
  teardown_timeout_milliseconds | Positive,
} in
let Profile = {
  upstream_source | Exact "ssh://git@github.com/NixOS/nix-eval-jobs.git",
  upstream_revision | Exact "a0cd02231c58974a6b5aaa3712069b071047162e",
  upstream_license | Exact "GPL-3.0",
  adapter_version | Exact "2.35.0",
  source_version | Exact "0.1.6",
  nix_source | Exact "ssh://git@github.com/NixOS/nix.git",
  nix_revision | Exact "aa2613fa02ca6199fe0ebf61a0f7f6d781d32a47",
  nix_version | Exact "2.35.2",
  nixpkgs_revision | Exact "346dd96ad74dc4457a9db9de4f4f57dab2e5731d",
  platform | Exact "x86_64-linux",
  cohort_manifest_schema | Exact "kiln.nix-eval-jobs-cohort.v1",
  bounded_exec_source | Exact "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
  bounded_exec_revision | Exact "29dac88ecded94457572db3fdfaaaab95fa91525",
  bounded_exec_license | Exact "AGPL-3.0-or-later",
  provider_profile_id | Exact "nix-eval-jobs-v1",
  provider_role | Exact "discovery",
  capabilities | Exact ["cancel", "discover", "observe", "reconcile"],
  outcomes | Exact ["cancelled", "domain_failure", "provider_error", "success", "timed_out", "unknown"],
  executable_path | Exact "/nix/store/bcv7hkwwpmfjmvxh6i106gdwr956svsp-nix-eval-jobs-2.35.0/bin/nix-eval-jobs",
  executable_identity | Exact "dc42bc0e330c598ec93c2361afbb493417fa5c3364722897bd156d3f2ba9c063",
  cohort_manifest_path | Exact "/nix/store/wasviibxqrxh6h8wckaifm2s6lancgpn-kiln-nix-eval-jobs-cohort.json",
  cohort_manifest_identity | Exact "102b4af9a753d00dfb6489e1593dd4b9ac8ffe9eed42518705329c6018e63c31",
  gc_root_base | AbsolutePath,
  home_base | AbsolutePath,
  policy | Policy,
  limits | Limits,
  required_non_claims | Exact [
    "discovery observation does not prove Nix evaluation correctness",
    "source identity does not prove source trust",
    "task mapping does not prove complete dependency semantics",
    "cache advisory does not prove cache truth or output presence",
    "discovery observation does not prove build execution or output correctness",
    "stock Nix cohort evidence does not prove Onix Nix compatibility",
    "discovery observation does not prove reproducibility",
    "discovery observation does not establish release readiness",
  ],
} in
let MaxLineBytes = 1048576 in
let MaxTotalStdoutBytes = 16777216 in
let MaxStderrBytes = 1048576 in
let MaxRecords = 256 in
let MaxJobs = 256 in
let MaxJobErrors = 256 in
let MaxMetadataBytes = 65536 in
let MaxDiagnosticBytes = 65536 in
let MaxOutputsPerJob = 64 in
let MaxDependenciesPerJob = 256 in
let MaxTasks = 256 in
let MaxGcRoots = 256 in
let MaxExecutableBytes = 536870912 in
let MaxManifestBytes = 65536 in
let MaxInFlight = 16 in
let MaxWorkers = 32 in
let MaxMemoryMebibytes = 16384 in
let StdinMaxBytes = 1 in
let TimeoutMilliseconds = 3600000 in
let PollIntervalMilliseconds = 10 in
let TeardownTimeoutMilliseconds = 1000 in
{
  profile | Profile = {
    upstream_source = "ssh://git@github.com/NixOS/nix-eval-jobs.git",
    upstream_revision = "a0cd02231c58974a6b5aaa3712069b071047162e",
    upstream_license = "GPL-3.0",
    adapter_version = "2.35.0",
    source_version = "0.1.6",
    nix_source = "ssh://git@github.com/NixOS/nix.git",
    nix_revision = "aa2613fa02ca6199fe0ebf61a0f7f6d781d32a47",
    nix_version = "2.35.2",
    nixpkgs_revision = "346dd96ad74dc4457a9db9de4f4f57dab2e5731d",
    platform = "x86_64-linux",
    cohort_manifest_schema = "kiln.nix-eval-jobs-cohort.v1",
    bounded_exec_source = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
    bounded_exec_license = "AGPL-3.0-or-later",
    provider_profile_id = "nix-eval-jobs-v1",
    provider_role = "discovery",
    capabilities = ["cancel", "discover", "observe", "reconcile"],
    outcomes = ["cancelled", "domain_failure", "provider_error", "success", "timed_out", "unknown"],
    executable_path = "/nix/store/bcv7hkwwpmfjmvxh6i106gdwr956svsp-nix-eval-jobs-2.35.0/bin/nix-eval-jobs",
    executable_identity = "dc42bc0e330c598ec93c2361afbb493417fa5c3364722897bd156d3f2ba9c063",
    cohort_manifest_path = "/nix/store/wasviibxqrxh6h8wckaifm2s6lancgpn-kiln-nix-eval-jobs-cohort.json",
    cohort_manifest_identity = "102b4af9a753d00dfb6489e1593dd4b9ac8ffe9eed42518705329c6018e63c31",
    gc_root_base = "/var/lib/kiln/nix-eval-roots",
    home_base = "/var/lib/kiln/nix-eval-home",
    policy = {
      flake_mode = true,
      expression_mode = false,
      allow_apply = false,
      allow_impure = false,
      allow_force_recurse = false,
      allow_registries = false,
      retain_gc_roots = true,
      partial_admission = false,
      check_cache_status = true,
      include_metadata = true,
      include_input_derivations = true,
    },
    limits = {
      max_line_bytes = MaxLineBytes,
      max_total_stdout_bytes = MaxTotalStdoutBytes,
      max_stderr_bytes = MaxStderrBytes,
      max_records = MaxRecords,
      max_jobs = MaxJobs,
      max_job_errors = MaxJobErrors,
      max_metadata_bytes = MaxMetadataBytes,
      max_diagnostic_bytes = MaxDiagnosticBytes,
      max_outputs_per_job = MaxOutputsPerJob,
      max_dependencies_per_job = MaxDependenciesPerJob,
      max_tasks = MaxTasks,
      max_gc_roots = MaxGcRoots,
      max_executable_bytes = MaxExecutableBytes,
      max_manifest_bytes = MaxManifestBytes,
      max_in_flight = MaxInFlight,
      max_workers = MaxWorkers,
      max_memory_mebibytes = MaxMemoryMebibytes,
      stdin_max_bytes = StdinMaxBytes,
      timeout_milliseconds = TimeoutMilliseconds,
      poll_interval_milliseconds = PollIntervalMilliseconds,
      teardown_timeout_milliseconds = TeardownTimeoutMilliseconds,
    },
    required_non_claims = [
      "discovery observation does not prove Nix evaluation correctness",
      "source identity does not prove source trust",
      "task mapping does not prove complete dependency semantics",
      "cache advisory does not prove cache truth or output presence",
      "discovery observation does not prove build execution or output correctness",
      "stock Nix cohort evidence does not prove Onix Nix compatibility",
      "discovery observation does not prove reproducibility",
      "discovery observation does not establish release readiness",
    ],
  },
}
