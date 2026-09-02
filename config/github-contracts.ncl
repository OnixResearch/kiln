# Shared human-authoring contracts; Rust admission remains authoritative.
let BoundedIntegerContract = fun minimum maximum => std.contract.from_predicate (fun value =>
  std.is_number value && value >= minimum && value <= maximum
  && std.number.floor value == value
) in
let PositiveContract = BoundedIntegerContract 1 9007199254740991 in
let NonEmptyContract = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let AbsolutePathContract = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^/[^\\x00-\\x1f\\x7f]+$" value
  && !(std.string.is_match "(^|/)\\.\\.(/|$)" value)
) in
let Blake3Contract = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let ComponentContract = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value <= 100
  && std.string.is_match "^[a-z0-9_.-]+$" value && value != "." && value != ".."
) in
let OwnerContract = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value <= 39
  && std.string.is_match "^[a-z0-9]([a-z0-9-]*[a-z0-9])?$" value
) in
let SecretName = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value <= 128
  && std.string.is_match "^[a-zA-Z0-9_.][a-zA-Z0-9_.-]*$" value
) in
let NullableContract = fun Contract => std.contract.custom (fun label value =>
  if value == null then 'Ok value else std.contract.check Contract label value
) in
let SecretReferenceContract = {
  program_path | AbsolutePathContract,
  program_blake3 | Blake3Contract,
  project_root | AbsolutePathContract,
  profile | SecretName,
  provider | NullableContract SecretName,
  secret_key | SecretName,
} in
let Node = {
  id | NonEmptyContract,
  kind | NonEmptyContract,
  effect_class | std.contract.from_predicate (fun value => value == "build" || value == "test"),
  adapter_kind | NonEmptyContract,
  operation_identity | Blake3Contract,
} in
let WorkflowContract = {
  socket_path | AbsolutePathContract,
  allowed_user_ids | Array (BoundedIntegerContract 0 4294967295),
  transport_limits | {
    maximum_frame_bytes | PositiveContract,
    maximum_log_bytes | PositiveContract,
    maximum_connections | PositiveContract,
    read_deadline_milliseconds | PositiveContract,
    write_deadline_milliseconds | PositiveContract,
  },
  adapter_limits | { maximum_request_bytes | PositiveContract, maximum_response_bytes | PositiveContract },
  policy_id | NonEmptyContract,
  provider_profile_id | NonEmptyContract,
  platform | NonEmptyContract,
  nodes | Array Node,
  edges | Array { source | NonEmptyContract, target | NonEmptyContract },
  controls | {
    cancel | Bool,
    pause | std.contract.from_predicate (fun value => value == false),
    resume | std.contract.from_predicate (fun value => value == false),
  },
  bounds | {
    max_nodes | PositiveContract,
    max_edges | PositiveContract,
    max_log_references | PositiveContract,
    max_log_read_bytes | PositiveContract,
    max_total_log_bytes | PositiveContract,
  },
} in
let RequiredNonClaims = [
  "webhook authentication does not prove delivery freshness",
  "replay admission does not prove distributed exactly-once execution",
  "source identity does not prove source trust or workflow correctness",
  "bounded jobs and temporary checkouts do not provide host confinement",
  "GitHub status publication does not replace canonical Kiln CI state",
  "local checks do not prove deployment or release eligibility",
] in
let ServiceProfileContract = {
  schema | std.contract.from_predicate (fun value => value == "kiln.github-ci.v1"),
  event_policy | {
    owner | OwnerContract,
    repository | ComponentContract,
    repository_id | PositiveContract,
    allow_fork_pull_requests | std.contract.from_predicate (fun value => value == false),
  },
  listen_address | std.contract.from_predicate (fun value => value == "127.0.0.1:8787"),
  webhook_path | std.contract.from_predicate (fun value => value == "/github/webhook"),
  replay_database_path | AbsolutePathContract,
  maximum_run_records | PositiveContract,
  maximum_delivery_records | PositiveContract,
  maximum_record_bytes | PositiveContract,
  maximum_body_bytes | BoundedIntegerContract 1 33554432,
  maximum_header_bytes | BoundedIntegerContract 1 65536,
  read_timeout_ms | BoundedIntegerContract 1 4000,
  write_timeout_ms | BoundedIntegerContract 1 4000,
  maximum_connections | BoundedIntegerContract 1 64,
  worker_poll_interval_ms | PositiveContract,
  maximum_workflow_polls | PositiveContract,
  maximum_active_runs | PositiveContract,
  webhook_secret | SecretReferenceContract,
  status_secret | SecretReferenceContract,
  status | {
    owner | OwnerContract,
    repository | ComponentContract,
    context | std.contract.from_predicate (fun value => value == "kiln/ci"),
    maximum_response_bytes | BoundedIntegerContract 1 4194304,
    maximum_pages | BoundedIntegerContract 1 100,
    timeout_ms | BoundedIntegerContract 1 120000,
  },
  workflow | WorkflowContract,
  non_claims | std.contract.from_predicate (fun value => value == RequiredNonClaims),
} in
let JobProfileContract = {
  schema_version | std.contract.from_predicate (fun value => value == 1),
  owner | OwnerContract,
  repository | ComponentContract,
  program_path | AbsolutePathContract,
  args | Array String,
  environment | { _ : String },
  source_secret | NullableContract SecretReferenceContract,
  snapshot_limits | {
    max_entries | PositiveContract,
    max_path_bytes | PositiveContract,
    max_file_bytes | PositiveContract,
    max_total_bytes | PositiveContract,
  },
  maximum_transfer_bytes | PositiveContract,
  acquisition_timeout_ms | PositiveContract,
  command_timeout_ms | PositiveContract,
  maximum_output_bytes | PositiveContract,
  teardown_grace_ms | PositiveContract,
} in
{
  BoundedInteger = BoundedIntegerContract,
  Positive = PositiveContract,
  NonEmpty = NonEmptyContract,
  AbsolutePath = AbsolutePathContract,
  Blake3 = Blake3Contract,
  Owner = OwnerContract,
  Component = ComponentContract,
  SecretReference = SecretReferenceContract,
  Nullable = NullableContract,
  Workflow = WorkflowContract,
  NonClaims = RequiredNonClaims,
  ServiceProfile = ServiceProfileContract,
  JobProfile = JobProfileContract,
}
