# r[impl kiln.radicle_ci.runtime]
# One reviewed document is hashed once; event source arrives only through
# LATTICE_SOURCE_PROVIDER/REPOSITORY/REVISION and controller source-intent.json.
# project-release PROFILE_JSON PROFILE_BLAKE3 SOURCE_CONTEXT_JSON uses an explicit
# { provider, repository, revision } selection, not ambient workflow variables.
# Source and build output roots remain owned until this explicit handoff/release.
let NonEmpty = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0) in
let Absolute = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^/[^\\x00]+$" value) in
let Positive = std.contract.from_predicate (fun value => std.is_number value && value > 0) in
let Branch = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^refs/heads/.+$" value) in
let Blake3 = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value) in
let Ref = { contract_version | Number, provider_namespace | NonEmpty, native_kind | NonEmpty, native_id | NonEmpty } in
let Intent = {
  effect_id | NonEmpty,
  attempt_id | NonEmpty,
  idempotency_key | Blake3,
  candidate_id | NonEmpty,
  pipeline_revision | NonEmpty,
  plan_identity | Blake3,
  policy_identity | Blake3,
  platform | NonEmpty,
}
in
let Eval = {
  platform | NonEmpty,
  runtime
    | {
      executable_path | Absolute,
      executable_identity | Blake3,
      cohort_manifest_path | Absolute,
      cohort_manifest_identity | Blake3,
      gc_root_base | Absolute,
      home_base | Absolute,
    },
  ..
}
in
let Build = {
  executable_path | Absolute,
  executable_identity | Blake3,
  nix_store_executable_path | Absolute,
  nix_store_executable_identity | Blake3,
  store_path | Absolute,
  gc_root_base | Absolute,
  home_base | Absolute,
  worker_identity | Blake3,
  store_identity | Blake3,
  plan_identity | Blake3,
  policy_identity | Blake3,
  ..
}
in
let Discovery = {
  effect_id | NonEmpty,
  evaluation_id | NonEmpty,
  project_id | NonEmpty,
  pipeline_revision_id | NonEmpty,
  discovery_policy_id | NonEmpty,
  provider_profile | NonEmpty,
  build_provider_profile | NonEmpty,
  idempotency_key | Blake3,
  source_identity | Blake3,
  source | Ref,
  source_mode | std.contract.from_predicate (fun value => value == "Flake"),
  root_selector | NonEmpty,
  system | NonEmpty,
  options
    | {
      workers | Positive,
      max_memory_mebibytes | Positive,
      include_metadata | Bool,
      include_input_derivations | Bool,
      check_cache_status | Bool,
      partial_admission | Bool,
    },
  bounds
    | {
      max_records | Positive,
      max_jobs | Positive,
      max_job_errors | Positive,
      max_diagnostic_bytes | Positive,
      max_outputs_per_job | Positive,
      max_dependencies_per_job | Positive,
      max_tasks | Positive,
    },
  ..
}
in
let Limits = {
  max_identity_bytes | Positive,
  max_reference_bytes | Positive,
  max_entries | Positive,
  max_path_bytes | Positive,
  max_file_bytes | Positive,
  max_total_bytes | Positive,
}
in
let Source = {
  repository_path | Absolute,
  repository_id | NonEmpty,
  signer_node_id | NonEmpty,
  signed_reference | Branch,
  profile_id | NonEmpty,
  limits | Limits,
}
in
# The exact native evaluator/build documents are supplied explicitly. Rust pure
# admission enforces their complete pinned cohorts and rejects mixed/fallback
# evaluators. In Onix mode use the existing worker eval-profile constructor;
# its source template is replaced only with measured acquired-source facts.
fun inputs =>
  let selected
    | {
      source | Source,
      state_root | Absolute,
      discovery_profile | Eval,
      discovery_plan | Discovery,
      selected_task | NonEmpty,
      build_profile | Build,
      build_intent | Intent,
    }
    = inputs
  in
  {
    schema = "kiln.radicle-ci.project-worker.v1",
    source = selected.source,
    state_root = selected.state_root,
    discovery_profile = selected.discovery_profile,
    discovery_plan = selected.discovery_plan,
    selected_task = selected.selected_task,
    build_profile = selected.build_profile,
    build_intent = selected.build_intent,
  }
