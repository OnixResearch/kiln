# Human-authoring contracts for the Tangled spindle profile.
# Rust admission (`kiln_adapter_tangled::profile::admit_profile`) stays authoritative.
let BoundedIntegerContract = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= minimum
    && value <= maximum
    && std.number.floor value == value
  )
in
let PositiveContract = BoundedIntegerContract 1 9007199254740991 in
let NonEmptyContract =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0 && std.string.length value <= 4096
  )
in
let IdentifierContract =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9][A-Za-z0-9._:-]*$" value
  )
in
let AbsolutePathContract =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/[^\\x00-\\x1f\\x7f]+$" value
    && !(std.string.is_match "(^|/)\\.\\.(/|$)" value)
  )
in
let HttpsUrlContract =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 4096
    && std.string.is_match "^https://[^/@?#\\s]+(/[^?#\\s]*)?$" value
  )
in
let DidContract =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 2048
    && std.string.is_match "^did:[a-z]+:[A-Za-z0-9._:%-]*[A-Za-z0-9._-]$" value
  )
in
let WebDidContract =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^did:web:[a-z0-9]([a-z0-9.-]*[a-z0-9])?(%3[Aa][0-9]{1,5})?$" value
  )
in
let RequiredNonClaims = [
  "Tangled admission proves a verified knot or actor signature, not source trust",
  "Tangled workflow files run only through the reviewed Kiln mapping",
  "Tangled status deliveries are Kiln projections, not Tangled records",
]
in
let RepositoryContract = {
  repo_did | DidContract,
  knot_did | WebDidContract,
  clone_url | HttpsUrlContract,
  trusted_actors | Array DidContract,
  approvers | Array DidContract | std.contract.from_predicate (fun values => std.array.length values > 0),
}
in
let EngineContract = {
  engine | NonEmptyContract,
  images | Array NonEmptyContract,
  packages | Array NonEmptyContract,
  platform | IdentifierContract,
  adapter_kind | IdentifierContract,
}
in
let WorkflowContract = {
  policy_id | IdentifierContract,
  untrusted_policy_id | IdentifierContract,
  provider_profile_id | IdentifierContract,
  controls
    | {
      cancel | Bool,
      pause | std.contract.from_predicate (fun value => value == false),
      resume | std.contract.from_predicate (fun value => value == false),
    },
  bounds
    | {
      max_nodes | PositiveContract,
      max_edges | PositiveContract,
      max_log_references | PositiveContract,
      max_log_read_bytes | PositiveContract,
      max_total_log_bytes | PositiveContract,
    },
}
in
let LimitsContract = {
  maximum_body_bytes | BoundedIntegerContract 1 33554432,
  maximum_header_bytes | BoundedIntegerContract 1 65536,
  read_timeout_ms | BoundedIntegerContract 1 60000,
  write_timeout_ms | BoundedIntegerContract 1 60000,
  maximum_token_lifetime_seconds | BoundedIntegerContract 1 3600,
  clock_skew_seconds | BoundedIntegerContract 0 600,
  maximum_tokens | PositiveContract,
  maximum_pipelines | PositiveContract,
  maximum_record_bytes | PositiveContract,
  maximum_workflow_files | BoundedIntegerContract 1 1024,
  maximum_workflow_bytes | PositiveContract,
  maximum_changed_paths | PositiveContract,
  maximum_changed_path_bytes | PositiveContract,
  acquisition_timeout_ms | PositiveContract,
  resolver_timeout_ms | PositiveContract,
  maximum_did_document_bytes | PositiveContract,
  key_cache_seconds | PositiveContract,
  maximum_active_runs | PositiveContract,
  maximum_workflow_polls | PositiveContract,
  worker_poll_interval_ms | PositiveContract,
}
in
let ProfileContract = {
  schema | std.contract.from_predicate (fun value => value == "kiln.tangled-spindle.v1"),
  spindle_did | WebDidContract,
  listen_address | NonEmptyContract,
  database_path | AbsolutePathContract,
  source_directory | AbsolutePathContract,
  git_program | AbsolutePathContract,
  plc_directory_url | HttpsUrlContract,
  repositories | Array RepositoryContract | std.contract.from_predicate (fun values => std.array.length values > 0),
  engines | Array EngineContract | std.contract.from_predicate (fun values => std.array.length values > 0),
  workflow | WorkflowContract,
  status
    | {
      endpoint | HttpsUrlContract,
      secret_path | AbsolutePathContract,
      timeout_ms | BoundedIntegerContract 1 120000,
      maximum_attempts | BoundedIntegerContract 1 100,
    },
  limits | LimitsContract,
  non_claims | std.contract.from_predicate (fun value => value == RequiredNonClaims),
}
in
{
  Positive = PositiveContract,
  AbsolutePath = AbsolutePathContract,
  HttpsUrl = HttpsUrlContract,
  Did = DidContract,
  WebDid = WebDidContract,
  Identifier = IdentifierContract,
  Repository = RepositoryContract,
  Engine = EngineContract,
  Workflow = WorkflowContract,
  NonClaims = RequiredNonClaims,
  Profile = ProfileContract,
}
