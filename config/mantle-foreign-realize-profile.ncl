# r[impl kiln.mantle.foreign_realize.profile] r[impl kiln.mantle.foreign_realize.nonclaims]
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
let LogicalPath =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^/nix/store/[^/]" value
  )
in
let Blake3 =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Binding = {
  plan_path | AbsolutePath,
  plan_file_identity | Blake3,
  plan_identity | Blake3,
  import_receipt_path | AbsolutePath,
  import_receipt_file_identity | Blake3,
  import_receipt_identity | Blake3,
  source_bundle_path | AbsolutePath,
  source_bundle_file_identity | Blake3,
  source_bundle_manifest_identity | Blake3,
  execution_profile_path | AbsolutePath,
  execution_profile_file_identity | Blake3,
  cache_closure_policy_path | AbsolutePath,
  cache_closure_policy_file_identity | Blake3,
  cache_closure_policy_identity | Blake3,
  remote_binding_config_path | AbsolutePath,
  remote_binding_config_file_identity | Blake3,
  product_roots | { _ | LogicalPath },
}
in
let Profile = {
  source | Exact "ssh://git@github.com/OnixResearch/mantle.git",
  revision | Exact "ead7b8dac087fad0170daca01356afbb3b43b179",
  contract_license | Exact "MIT OR Apache-2.0",
  cli_license | Exact "AGPL-3.0-or-later",
  bounded_exec_source | Exact "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
  bounded_exec_revision | Exact "29dac88ecded94457572db3fdfaaaab95fa91525",
  bounded_exec_license | Exact "AGPL-3.0-or-later",
  plan_schema | Exact "mantle-foreign-executable-plan-v1",
  import_receipt_schema | Exact "foreign-derivation-import-receipt-v1",
  receipt_schema | Exact "mantle-foreign-realization-receipt-v1",
  command | Exact "foreign-import realize",
  platform | Exact "x86_64-linux",
  provider_profile_id | Exact "mantle-foreign-realize-v1",
  capabilities | Exact ["cancel", "dispatch", "observe", "realize", "reconcile"],
  executable_path | Exact "/nix/store/PLACEHOLDER-mantle-0.1.0/bin/mantle",
  executable_identity | Exact "0000000000000000000000000000000000000000000000000000000000000000",
  store_prefix | Exact "/nix/store",
  store_path | AbsolutePath,
  state_path | AbsolutePath,
  limits
    | {
      max_required_products | Positive,
      max_products | Positive,
      max_bindings | Positive,
      max_roots | Exact 1,
      max_request_bytes | Positive,
      max_result_bytes | Positive,
      max_stderr_bytes | Positive,
      max_receipt_bytes | Positive,
      max_input_bytes | Positive,
      max_executable_bytes | Positive,
      max_in_flight | Positive,
      stdin_max_bytes | Positive,
      timeout_milliseconds | Positive,
      poll_interval_milliseconds | Positive,
      teardown_timeout_milliseconds | Positive,
    },
  required_non_claims
    | Exact [
      "realization observation does not prove realization correctness",
      "realization observation does not prove cache truth",
      "realization observation does not prove signature or key authority beyond Mantle's checks",
      "product admission does not prove product semantics",
      "realization observation does not establish release readiness",
    ],
  bindings | Array Binding,
}
in
let MaxRequiredProducts = 64 in
let MaxProducts = 64 in
let MaxBindings = 256 in
let MaxRoots = 1 in
let MaxRequestBytes = 1048576 in
let MaxResultBytes = 67108864 in
let MaxStderrBytes = 1048576 in
let MaxReceiptBytes = 67108864 in
let MaxInputBytes = 67108864 in
let MaxExecutableBytes = 268435456 in
let MaxInFlight = 16 in
let StdinMaxBytes = 1 in
let TimeoutMilliseconds = 3600000 in
let PollIntervalMilliseconds = 10 in
let TeardownTimeoutMilliseconds = 1000 in
{
  profile | Profile = {
    source = "ssh://git@github.com/OnixResearch/mantle.git",
    revision = "ead7b8dac087fad0170daca01356afbb3b43b179",
    contract_license = "MIT OR Apache-2.0",
    cli_license = "AGPL-3.0-or-later",
    bounded_exec_source = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
    bounded_exec_license = "AGPL-3.0-or-later",
    plan_schema = "mantle-foreign-executable-plan-v1",
    import_receipt_schema = "foreign-derivation-import-receipt-v1",
    receipt_schema = "mantle-foreign-realization-receipt-v1",
    command = "foreign-import realize",
    platform = "x86_64-linux",
    provider_profile_id = "mantle-foreign-realize-v1",
    capabilities = ["cancel", "dispatch", "observe", "realize", "reconcile"],
    executable_path = "/nix/store/PLACEHOLDER-mantle-0.1.0/bin/mantle",
    executable_identity = "0000000000000000000000000000000000000000000000000000000000000000",
    store_prefix = "/nix/store",
    store_path = "/var/lib/kiln/mantle-realize-store",
    state_path = "/var/lib/kiln/mantle-realize-state",
    limits = {
      max_required_products = MaxRequiredProducts,
      max_products = MaxProducts,
      max_bindings = MaxBindings,
      max_roots = MaxRoots,
      max_request_bytes = MaxRequestBytes,
      max_result_bytes = MaxResultBytes,
      max_stderr_bytes = MaxStderrBytes,
      max_receipt_bytes = MaxReceiptBytes,
      max_input_bytes = MaxInputBytes,
      max_executable_bytes = MaxExecutableBytes,
      max_in_flight = MaxInFlight,
      stdin_max_bytes = StdinMaxBytes,
      timeout_milliseconds = TimeoutMilliseconds,
      poll_interval_milliseconds = PollIntervalMilliseconds,
      teardown_timeout_milliseconds = TeardownTimeoutMilliseconds,
    },
    required_non_claims = [
      "realization observation does not prove realization correctness",
      "realization observation does not prove cache truth",
      "realization observation does not prove signature or key authority beyond Mantle's checks",
      "product admission does not prove product semantics",
      "realization observation does not establish release readiness",
    ],
    bindings = [
      {
        plan_path = "/home/brittonr/wt/convergence-evidence/realization/mantle-graph-preserve-http/plan.json",
        plan_file_identity = "d4efeaa6657729d9800a4b9b1aac6836c37e3839a6483c8c9d3399e29f65ae45",
        plan_identity = "edd387e529e1d4b277d726edc93901889413dc0dc393c7c7fd715981afb08a78",
        import_receipt_path = "/home/brittonr/wt/convergence-evidence/realization/mantle-graph-preserve-http/import-receipt.json",
        import_receipt_file_identity = "16beddc450fa02626272aefd42d1f37cd0309fa7d4986a53c68c1d48e95c399f",
        import_receipt_identity = "60a452562c2d845ec379bef934a0d03b97004e8959b2c409bb4804f9a2558c39",
        source_bundle_path = "/datapool/mantle-store/convergence-pilot-20261119/source-bundle-http.json",
        source_bundle_file_identity = "b8abb366a886bb79314411a3d479a97fe8481173334c238333b9118fe7ab8030",
        source_bundle_manifest_identity = "8ed1103b5de3054ee13ea391af805e276e3cc3b6ceaa50b2149193adb1ff1777",
        execution_profile_path = "/var/lib/kiln/mantle-foreign-nix-v1.json",
        execution_profile_file_identity = "054ec6754f08d60088cf3388f43295ad37621e5115e979f702a2a5c5d0eefcb8",
        cache_closure_policy_path = "/home/brittonr/wt/convergence-evidence/realization/pilot-cache-closure-policy.json",
        cache_closure_policy_file_identity = "9e605fc1f94c372608452d8f49871384b077dd34fe6a748f3d3bfa0879c58194",
        cache_closure_policy_identity = "0d228941b966d100140a23dbd865f39a4d54579662747b42ec41f718832261a7",
        remote_binding_config_path = "/datapool/mantle-store/convergence-pilot-20261119/binding/remote-endpoint-binding.json",
        remote_binding_config_file_identity = "6eb65f6cef773b281319853d1ad548eddc0928a5b98b460142071d72589a21fb",
        product_roots = {
          out = "/nix/store/zjq6whhf2c641vkz89kgwwpbajjq5cbq-octet-mantle-aeneas-consumer-job-v1",
        },
      },
    ],
  },
}
