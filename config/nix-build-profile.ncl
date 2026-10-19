# r[impl kiln.nix_build.cohort] r[impl kiln.nix_build.nonclaims]
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^/" value
  )
in
let Blake3 =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Profile = {
  nix_version | Exact "2.36.0",
  bounded_exec_source | Exact "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
  bounded_exec_revision | Exact "29dac88ecded94457572db3fdfaaaab95fa91525",
  bounded_exec_license | Exact "AGPL-3.0-or-later",
  request_schema | Exact "kiln.nix-build-request.v1",
  observation_schema | Exact "kiln.nix-build-observation.v1",
  receipt_schema | Exact "kiln.nix-build-receipt.v1",
  native_report_schema | Exact "nix.build-json.v1",
  command | Exact "build",
  platform | Exact "x86_64-linux",
  provider_profile_id | Exact "nix-build-v1",
  capabilities | Exact ["cancel", "dispatch", "observe", "read_logs", "reconcile"],
  executable_path | Exact "/nix/store/6rwk5j1qqk7na4la5m2ka34p734braxa-nix-2.36.0/bin/nix",
  executable_identity | Exact "0e670a46c789a33272d9193080b91affe8023af27bcfa9fad76d3d959178a8e7",
  derivation_path | Exact "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-example.drv",
  derivation_identity | Blake3,
  plan_identity | Blake3,
  policy_identity | Blake3,
  store_path | Exact "/nix/store",
  home_base | AbsolutePath,
  worker_identity | Blake3,
  store_identity | Blake3,
  required_products | Exact ["out"],
  limits
    | {
      max_required_products | Positive,
      max_products | Positive,
      max_logs | Exact 1,
      max_request_bytes | Positive,
      max_result_bytes | Positive,
      max_stderr_bytes | Positive,
      max_log_bytes | Positive,
      max_executable_bytes | Positive,
      max_derivation_bytes | Positive,
      max_in_flight | Positive,
      stdin_max_bytes | Positive,
      timeout_milliseconds | Positive,
      poll_interval_milliseconds | Positive,
      teardown_timeout_milliseconds | Positive,
    },
  required_non_claims
    | Exact [
      "build observation does not prove build correctness",
      "build observation does not prove sandbox completeness",
      "cache observation does not prove cache truth",
      "product admission does not prove product semantics",
      "build observation does not prove reproducibility",
      "nix build execution does not prove derivation provenance or source trust",
      "build observation does not establish release readiness",
    ],
}
in
let MaxRequiredProducts = 16 in
let MaxProducts = 64 in
let MaxRequestBytes = 1048576 in
let MaxResultBytes = 16777216 in
let MaxStderrBytes = 1048576 in
let MaxLogBytes = 1048576 in
let MaxExecutableBytes = 268435456 in
let MaxDerivationBytes = 16777216 in
let MaxInFlight = 16 in
let StdinMaxBytes = 1 in
let TimeoutMilliseconds = 3600000 in
let PollIntervalMilliseconds = 10 in
let TeardownTimeoutMilliseconds = 1000 in
{
  profile | Profile = {
    nix_version = "2.36.0",
    bounded_exec_source = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
    bounded_exec_license = "AGPL-3.0-or-later",
    request_schema = "kiln.nix-build-request.v1",
    observation_schema = "kiln.nix-build-observation.v1",
    receipt_schema = "kiln.nix-build-receipt.v1",
    native_report_schema = "nix.build-json.v1",
    command = "build",
    platform = "x86_64-linux",
    provider_profile_id = "nix-build-v1",
    capabilities = ["cancel", "dispatch", "observe", "read_logs", "reconcile"],
    executable_path = "/nix/store/6rwk5j1qqk7na4la5m2ka34p734braxa-nix-2.36.0/bin/nix",
    executable_identity = "0e670a46c789a33272d9193080b91affe8023af27bcfa9fad76d3d959178a8e7",
    derivation_path = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-example.drv",
    derivation_identity = "2222222222222222222222222222222222222222222222222222222222222222",
    plan_identity = "3333333333333333333333333333333333333333333333333333333333333333",
    policy_identity = "4444444444444444444444444444444444444444444444444444444444444444",
    store_path = "/nix/store",
    home_base = "/var/lib/kiln/nix-home",
    worker_identity = "5555555555555555555555555555555555555555555555555555555555555555",
    store_identity = "6666666666666666666666666666666666666666666666666666666666666666",
    required_products = ["out"],
    limits = {
      max_required_products = MaxRequiredProducts,
      max_products = MaxProducts,
      max_logs = 1,
      max_request_bytes = MaxRequestBytes,
      max_result_bytes = MaxResultBytes,
      max_stderr_bytes = MaxStderrBytes,
      max_log_bytes = MaxLogBytes,
      max_executable_bytes = MaxExecutableBytes,
      max_derivation_bytes = MaxDerivationBytes,
      max_in_flight = MaxInFlight,
      stdin_max_bytes = StdinMaxBytes,
      timeout_milliseconds = TimeoutMilliseconds,
      poll_interval_milliseconds = PollIntervalMilliseconds,
      teardown_timeout_milliseconds = TeardownTimeoutMilliseconds,
    },
    required_non_claims = [
      "build observation does not prove build correctness",
      "build observation does not prove sandbox completeness",
      "cache observation does not prove cache truth",
      "product admission does not prove product semantics",
      "build observation does not prove reproducibility",
      "nix build execution does not prove derivation provenance or source trust",
      "build observation does not establish release readiness",
    ],
  },
}
