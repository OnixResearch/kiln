# Reviewed single-tenant input builder. Export as JSON; Rust admission is authoritative.
# No repository, native handler, credential or execution is created by this builder.
let c = import "github-contracts.ncl" in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let Workflow = {
  socket_path | c.AbsolutePath,
  allowed_user_ids | Array (c.BoundedInteger 0 4294967295),
  transport_limits
    | {
      maximum_frame_bytes | c.Positive,
      maximum_log_bytes | c.Positive,
      maximum_connections | c.Positive,
      read_deadline_milliseconds | c.Positive,
      write_deadline_milliseconds | c.Positive,
    },
  adapter_limits | { maximum_request_bytes | c.Positive, maximum_response_bytes | c.Positive },
  policy_id | c.NonEmpty,
  provider_profile_id | c.NonEmpty,
  platform | c.NonEmpty,
  nodes
    | Array {
      id | c.NonEmpty,
      kind | c.NonEmpty,
      effect_class | std.contract.from_predicate (fun value => value == "build" || value == "test"),
      adapter_kind | c.NonEmpty,
      operation_identity | c.Blake3,
    },
  edges | Array { source | c.NonEmpty, target | c.NonEmpty },
  controls | { cancel | Bool, pause | Bool, resume | Bool },
  bounds | { max_nodes | c.Positive, max_edges | c.Positive, max_log_references | c.Positive, max_log_read_bytes | c.Positive, max_total_log_bytes | c.Positive },
}
in
fun input =>
  let settings =
    input
      | {
        service_id | Identifier,
        listen_address | c.NonEmpty,
        origin | std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^https://[^/?#@ ]+$" value),
        tls | { certificate_path | c.AbsolutePath, private_key_path | c.AbsolutePath, min_version | std.contract.from_predicate (fun value => value == "TLS1.3") },
        operator_token_file | c.AbsolutePath,
        worker_token_file | c.AbsolutePath,
        state_directory | c.AbsolutePath,
        projects
          | Array {
            id | Identifier,
            label | c.NonEmpty,
            repository_path | c.AbsolutePath,
            reference | c.NonEmpty,
            source_limits | { max_entries | c.Positive, max_path_bytes | c.Positive, max_file_bytes | c.Positive, max_total_bytes | c.Positive },
            workflow | Workflow,
          },
        bounds
          | {
            max_request_bytes | c.BoundedInteger 1 67108864,
            max_response_bytes | c.BoundedInteger 1024 9007199254740991,
            max_header_bytes | c.BoundedInteger 1 65536,
            max_connections | c.BoundedInteger 1 64,
            timeout_seconds | c.Positive,
            poll_interval_ms | c.Positive,
            max_projects | c.Positive,
            max_runs | c.Positive,
            max_artifact_bytes | c.Positive,
            max_artifacts | c.Positive,
            max_run_artifact_bytes | c.Positive,
            max_total_artifact_bytes | c.Positive,
            max_log_read_bytes | c.Positive,
            max_requests | c.Positive,
            max_events_per_run | c.BoundedInteger 4 9007199254740991,
            max_state_bytes | c.BoundedInteger 1048576 9007199254740991,
          },
      }
  in
  {
    schema = "kiln.web-service.v1",
    service_id = settings.service_id,
    listen_address = settings.listen_address,
    origin = settings.origin,
    tls = settings.tls,
    operator_token_file = settings.operator_token_file,
    worker_token_file = settings.worker_token_file,
    state_directory = settings.state_directory,
    projects = settings.projects,
    bounds = settings.bounds,
  }
