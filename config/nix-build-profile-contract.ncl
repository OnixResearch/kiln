let Positive = std.contract.from_predicate (fun value => std.is_number value && value > 0) in
let AbsolutePath = std.contract.from_predicate (fun value => std.is_string value && std.string.length value <= 4096 && std.string.is_match "^/[A-Za-z0-9._/+?=-]+$" value) in
let Blake3 = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value) in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let PathParts = fun value => std.array.filter (fun part => part != "") (std.string.split "/" value) in
let DirectChild = fun root path =>
  let root_parts = PathParts root in
  let path_parts = PathParts path in
  std.array.length path_parts == std.array.length root_parts
  + 1
  && std.array.slice 0 (std.array.length root_parts) path_parts == root_parts
in
let OutputName = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0 && std.string.length value <= 64 && std.string.is_match "^[a-z][a-z0-9_-]*$" value) in
let OutputOrder =
  std.contract.from_predicate (fun outputs =>
    let names = std.array.map (fun output => output.name) outputs in
    let sorted = std.array.sort (fun left right => if left < right then 'Lesser else if left > right then 'Greater else 'Equal) names in
    let unique = std.array.fold_left (fun seen name => if std.array.elem name seen then seen else seen @ [name]) [] names in
    std.array.length outputs >= 1 && std.array.length outputs <= 64 && names == sorted && std.array.length names == std.array.length unique
  )
in
let ProfileRelations =
  std.contract.from_predicate (fun profile =>
    let safe_parts = fun value => std.array.all (fun part => part != "." && part != "..") (PathParts value) in
    safe_parts profile.store_path
    && safe_parts profile.derivation_path
    && DirectChild profile.store_path profile.derivation_path
    && std.string.is_match "\\.drv$" profile.derivation_path
    && std.array.all (fun output => safe_parts output.path && DirectChild profile.store_path output.path) profile.expected_outputs
    && std.array.length profile.expected_outputs <= profile.limits.max_required_products
    && std.array.length profile.expected_outputs <= profile.limits.max_products
  )
in
let ProfileShape = {
  nixpkgs_revision | Exact "346dd96ad74dc4457a9db9de4f4f57dab2e5731d",
  nix_version | Exact "2.34.8",
  bounded_exec_source | Exact "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
  bounded_exec_revision | Exact "29dac88ecded94457572db3fdfaaaab95fa91525",
  bounded_exec_license | Exact "AGPL-3.0-or-later",
  request_schema | Exact "kiln.nix-build-request.v1",
  observation_schema | Exact "kiln.nix-build-observation.v1",
  receipt_schema | Exact "kiln.nix-build-receipt.v1",
  native_report_schema | Exact "nix-store-realisations.v1",
  command | Exact "realise",
  platform | Exact "x86_64-linux",
  provider_profile_id | Exact "nix-build-v1",
  capabilities | Exact ["cancel", "dispatch", "observe", "read_logs", "reconcile"],
  executable_path | AbsolutePath,
  executable_identity | Blake3,
  nix_store_executable_path | AbsolutePath,
  nix_store_executable_identity | Blake3,
  derivation_path | AbsolutePath,
  derivation_identity | Blake3,
  plan_identity | Blake3,
  policy_identity | Blake3,
  store_path | AbsolutePath,
  store_root | AbsolutePath | optional,
  gc_root_base | AbsolutePath,
  home_base | AbsolutePath,
  worker_identity | Blake3,
  store_identity | Blake3,
  expected_outputs | Array { name | OutputName, path | AbsolutePath } | OutputOrder,
  rebuild_check | Bool | optional,
  limits
    | {
      max_required_products | Positive,
      max_products | Positive,
      max_logs | Exact 1,
      max_gc_roots | Positive,
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
{ Profile = std.contract.Sequence [ProfileShape, ProfileRelations] }
