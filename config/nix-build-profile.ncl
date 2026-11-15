let Profile = (import "nix-build-profile-contract.ncl").Profile in
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
    nixpkgs_revision = "346dd96ad74dc4457a9db9de4f4f57dab2e5731d",
    nix_version = "2.34.8",
    bounded_exec_source = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
    bounded_exec_license = "AGPL-3.0-or-later",
    request_schema = "kiln.nix-build-request.v1",
    observation_schema = "kiln.nix-build-observation.v1",
    receipt_schema = "kiln.nix-build-receipt.v1",
    native_report_schema = "nix-store-realisations.v1",
    command = "realise",
    platform = "x86_64-linux",
    provider_profile_id = "nix-build-v1",
    capabilities = ["cancel", "dispatch", "observe", "read_logs", "reconcile"],
    executable_path = "/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix",
    executable_identity = "82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852",
    nix_store_executable_path = "/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix-store",
    nix_store_executable_identity = "82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852",
    derivation_path = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-example.drv",
    derivation_identity = "2222222222222222222222222222222222222222222222222222222222222222",
    plan_identity = "3333333333333333333333333333333333333333333333333333333333333333",
    policy_identity = "4444444444444444444444444444444444444444444444444444444444444444",
    store_path = "/nix/store",
    gc_root_base = "/var/lib/kiln/nix-build-roots",
    home_base = "/var/lib/kiln/nix-home",
    worker_identity = "5555555555555555555555555555555555555555555555555555555555555555",
    store_identity = "6666666666666666666666666666666666666666666666666666666666666666",
    expected_outputs = [
      { name = "out", path = "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-example" },
    ],
    limits = {
      max_required_products = MaxRequiredProducts,
      max_products = MaxProducts,
      max_logs = 1,
      max_gc_roots = 256,
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
