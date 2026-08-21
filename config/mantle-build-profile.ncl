# r[impl kiln.mantle.profile] r[impl kiln.mantle.nonclaims]
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
let Profile = {
  source | Exact "ssh://git@github.com/OnixResearch/mantle.git",
  revision | Exact "0bbab77e23ab95caa4e4184b9945cfaa5c8040e2",
  contract_license | Exact "MIT OR Apache-2.0",
  cli_license | Exact "AGPL-3.0-or-later",
  bounded_exec_source | Exact "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
  bounded_exec_revision | Exact "29dac88ecded94457572db3fdfaaaab95fa91525",
  bounded_exec_license | Exact "AGPL-3.0-or-later",
  request_schema | Exact "mantle.build-request.v1",
  observation_schema | Exact "mantle.build-observation.v1",
  receipt_schema | Exact "mantle.build-receipt.v1",
  native_report_schema | Exact "crunch-build-report-v1",
  command | Exact "build",
  platform | Exact "x86_64-linux",
  provider_profile_id | Exact "mantle-build-v1",
  capabilities | Exact ["cancel", "dispatch", "observe", "read_logs", "reconcile"],
  executable_path | Exact "/nix/store/s16cv8qj77zsp2wg03qlvxj577frb6jr-mantle-0.1.0/bin/mantle",
  executable_identity | Exact "d8090e908e05f8fbd3c1154a77b95db9d20294a17a47856669199b11457c35c8",
  plan_path | AbsolutePath,
  plan_identity | Blake3,
  policy_identity | Blake3,
  store_path | AbsolutePath,
  state_path | AbsolutePath,
  store_identity_path | AbsolutePath,
  builder_identity | Blake3,
  worker_identity | Blake3,
  store_identity | Blake3,
  limits | {
    max_required_products | Positive,
    max_products | Positive,
    max_logs | Positive,
    max_metrics | Positive,
    max_request_bytes | Positive,
    max_result_bytes | Positive,
    max_stderr_bytes | Positive,
    max_log_bytes | Positive,
    max_executable_bytes | Positive,
    max_plan_bytes | Positive,
    max_store_identity_bytes | Positive,
    max_in_flight | Positive,
    stdin_max_bytes | Positive,
    timeout_milliseconds | Positive,
    poll_interval_milliseconds | Positive,
    teardown_timeout_milliseconds | Positive,
  },
  required_non_claims | Exact [
    "build observation does not prove build correctness",
    "build observation does not prove sandbox completeness",
    "cache observation does not prove cache truth",
    "product admission does not prove product semantics",
    "build observation does not prove reproducibility",
    "build observation does not establish release readiness",
  ],
} in
let MaxRequiredProducts = 256 in
let MaxProducts = 256 in
let MaxLogs = 256 in
let MaxMetrics = 256 in
let MaxRequestBytes = 1048576 in
let MaxResultBytes = 16777216 in
let MaxStderrBytes = 1048576 in
let MaxLogBytes = 1048576 in
let MaxExecutableBytes = 268435456 in
let MaxPlanBytes = 16777216 in
let MaxStoreIdentityBytes = 65536 in
let MaxInFlight = 64 in
let StdinMaxBytes = 1 in
let TimeoutMilliseconds = 3600000 in
let PollIntervalMilliseconds = 10 in
let TeardownTimeoutMilliseconds = 1000 in
{
  profile | Profile = {
    source = "ssh://git@github.com/OnixResearch/mantle.git",
    revision = "0bbab77e23ab95caa4e4184b9945cfaa5c8040e2",
    contract_license = "MIT OR Apache-2.0",
    cli_license = "AGPL-3.0-or-later",
    bounded_exec_source = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
    bounded_exec_license = "AGPL-3.0-or-later",
    request_schema = "mantle.build-request.v1",
    observation_schema = "mantle.build-observation.v1",
    receipt_schema = "mantle.build-receipt.v1",
    native_report_schema = "crunch-build-report-v1",
    command = "build",
    platform = "x86_64-linux",
    provider_profile_id = "mantle-build-v1",
    capabilities = ["cancel", "dispatch", "observe", "read_logs", "reconcile"],
    executable_path = "/nix/store/s16cv8qj77zsp2wg03qlvxj577frb6jr-mantle-0.1.0/bin/mantle",
    executable_identity = "d8090e908e05f8fbd3c1154a77b95db9d20294a17a47856669199b11457c35c8",
    plan_path = "/etc/kiln/mantle-plan.ncl",
    plan_identity = "2a44f29408b31e620e9247134f0b7b8d55012c8ae286de29199eecef33e10473",
    policy_identity = "6a094f32254723850459379582ef03c8d2dad65c25326ca526ba3fe62aa9d726",
    store_path = "/var/lib/kiln/mantle-store",
    state_path = "/var/lib/kiln/mantle-state",
    store_identity_path = "/var/lib/kiln/mantle-state/store-identity.json",
    builder_identity = "d8090e908e05f8fbd3c1154a77b95db9d20294a17a47856669199b11457c35c8",
    worker_identity = "6f66ef1d446ead1d87f37176335a3876e6c98af108febf4ec98600b3a91a8d4e",
    store_identity = "4bd89c0869c39e8d17e5a6799f977bdac78bbc72c93d7b97cb8348b5d3f69b57",
    limits = {
      max_required_products = MaxRequiredProducts,
      max_products = MaxProducts,
      max_logs = MaxLogs,
      max_metrics = MaxMetrics,
      max_request_bytes = MaxRequestBytes,
      max_result_bytes = MaxResultBytes,
      max_stderr_bytes = MaxStderrBytes,
      max_log_bytes = MaxLogBytes,
      max_executable_bytes = MaxExecutableBytes,
      max_plan_bytes = MaxPlanBytes,
      max_store_identity_bytes = MaxStoreIdentityBytes,
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
      "build observation does not establish release readiness",
    ],
  },
}
