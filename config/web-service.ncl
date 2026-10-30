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
# Monorepo scope limits mirror Rust admission.
let max_scope_patterns = 32 in
let max_scope_pattern_bytes = 256 in
let max_projects_per_repository = 64 in
# Unpadded base64 emits four symbols per three UTF-8 bytes, giving exact byte lengths.
let base64_bytes_per_symbol = 3 / 4 in
let utf8_bytes = fun value => std.number.floor (std.string.length (std.string.base64_encode 'NoPad value) * base64_bytes_per_symbol) in
# Repository-relative `/` path or glob: no absolute, empty, `.` or `..` segment, backslash or NUL.
let is_relative_path = fun value =>
  std.is_string value
  && utf8_bytes value <= max_scope_pattern_bytes
  # `std.string.split` drops a trailing empty segment, so require a final non-`/` character.
  && std.string.is_match "[^/]$" value
  && !(std.string.is_match "[\\x00\\\\]" value)
  && std.array.all (fun segment => segment != "" && segment != "." && segment != "..") (std.string.split "/" value)
in
let ScopeRoot = std.contract.from_predicate (fun value => value == "" || is_relative_path value) in
let ScopePattern = std.contract.from_predicate is_relative_path in
let PatternList = std.contract.from_predicate (fun value => std.is_array value && std.array.length value <= max_scope_patterns) in
let GraphKind = std.contract.from_predicate (fun value => value == "cargo" || value == "nix") in
let DistinctGraphs =
  std.contract.from_predicate (fun value =>
    std.is_array value
    && std.array.all (fun kind => std.array.length (std.array.filter (fun other => other == kind) value) == 1) value
  )
in
let Scope = {
  root | ScopeRoot,
  # Quoted: `include` is a Nickel keyword.
  "include" | Array ScopePattern | PatternList | default = ["**"],
  exclude | Array ScopePattern | PatternList | default = [],
  depends_on | Array ScopePattern | PatternList | default = [],
  graphs | Array GraphKind | DistinctGraphs | default = [],
}
in
# Lexically equal repository paths must be spelled identically, and each repository holds a bounded number of projects.
let repository_key = fun path => std.string.join "/" (std.array.filter (fun segment => segment != "" && segment != ".") (std.string.split "/" path)) in
let RepositoryProjects =
  std.contract.from_validator (fun projects =>
    let offending =
      std.array.filter
        (fun project =>
          let key = repository_key project.repository_path in
          let peers = std.array.filter (fun other => repository_key other.repository_path == key) projects in
          std.array.length peers > max_projects_per_repository
          || !(std.array.all (fun other => other.repository_path == project.repository_path) peers)
        )
        projects
    in
    if offending == [] then
      'Ok
    else
      'Error {
        message = "projects sharing a repository must use one identical repository_path and number at most %{std.string.from_number max_projects_per_repository}: %{(std.array.first offending).repository_path}"
      }
  )
in
let Project = {
  id | Identifier,
  label | c.NonEmpty,
  repository_path | c.AbsolutePath,
  reference | c.NonEmpty,
  admitted_references | Array c.NonEmpty | default = [],
  source_limits | { max_entries | c.Positive, max_path_bytes | c.Positive, max_file_bytes | c.Positive, max_total_bytes | c.Positive },
  workflow | Workflow,
  scope | Scope | optional,
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
        projects | Array Project | RepositoryProjects,
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
