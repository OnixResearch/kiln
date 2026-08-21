# r[verify kiln.nix_eval_jobs.conformance]
let Source = import "../nix-eval-jobs-profile.ncl" in
let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let StorePath = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^/nix/store/[0-9a-z]{32}-" value
) in
let Blake3 = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let WorkerCount = 2 in
let MemoryMebibytes = 512 in
let Request = {
  effect_id | NonEmpty,
  evaluation_id | NonEmpty,
  project_id | NonEmpty,
  pipeline_revision_id | NonEmpty,
  discovery_policy_id | NonEmpty,
  provider_profile_id | Exact "nix-eval-jobs-v1",
  build_provider_profile_id | NonEmpty,
  idempotency_key | Blake3,
  source_identity | Blake3,
  source_path | StorePath,
  source_mode | Exact "flake",
  root_selector | NonEmpty,
  system | Exact "x86_64-linux",
  workers | Positive,
  max_memory_mebibytes | Positive,
  partial_admission | Exact false,
} in
{
  profile = Source.profile,
  request | Request = {
    effect_id = "nix-discovery-one",
    evaluation_id = "evaluation-one",
    project_id = "project-one",
    pipeline_revision_id = "pipeline-one",
    discovery_policy_id = "nix-discovery-policy-one",
    provider_profile_id = "nix-eval-jobs-v1",
    build_provider_profile_id = "mantle-build-v1",
    idempotency_key = "1111111111111111111111111111111111111111111111111111111111111111",
    source_identity = "2222222222222222222222222222222222222222222222222222222222222222",
    source_path = "/nix/store/00000000000000000000000000000000-source",
    source_mode = "flake",
    root_selector = "jobs",
    system = "x86_64-linux",
    workers = WorkerCount,
    max_memory_mebibytes = MemoryMebibytes,
    partial_admission = false,
  },
}
