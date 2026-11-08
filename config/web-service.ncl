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
let ListenAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^([0-9]{1,3}\\.){3}[0-9]{1,3}:[1-9][0-9]*$|^\\[[0-9a-fA-F:]+\\]:[1-9][0-9]*$" value
  )
in
let AllowedUsers =
  std.contract.from_predicate (fun values =>
    let sorted =
      std.array.sort
        (fun left right =>
          if left < right then 'Lesser else if left > right then 'Greater else 'Equal
        )
        values
    in
    std.array.length values >= 1
    && std.array.length values <= 64
    && sorted == values
    && std.array.length values == std.array.length (
      std.array.fold_left
        (fun seen value => if std.array.elem value seen then seen else seen @ [value])
        []
        values
    )
  )
in
let WorkflowRelations =
  std.contract.from_predicate (fun workflow =>
    let unique = fun values =>
      std.array.length values == std.array.length (
        std.array.fold_left
          (fun seen value => if std.array.elem value seen then seen else seen @ [value])
          []
          values
      )
    in
    let ids = std.array.map (fun node => node.id) workflow.nodes in
    let keys = std.array.map (fun edge => edge.source ++ "\u{0}" ++ edge.target) workflow.edges in
    unique ids
    && unique keys
    && std.array.all
      (fun edge => std.array.elem edge.source ids && std.array.elem edge.target ids)
      workflow.edges
    && std.array.length workflow.nodes <= workflow.bounds.max_nodes
    && std.array.length workflow.edges <= workflow.bounds.max_edges
  )
in
let Workflow = {
  socket_path | c.AbsolutePath,
  allowed_user_ids | Array (c.BoundedInteger 0 4294967295) | AllowedUsers,
  transport_limits
    | {
      maximum_frame_bytes | c.BoundedInteger 1 33554432,
      maximum_log_bytes | c.BoundedInteger 1 16777216,
      maximum_connections | c.BoundedInteger 1 65536,
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
      effect_class | std.contract.from_predicate (fun value => value == "build" || value == "test" || value == "custom"),
      adapter_kind | c.NonEmpty,
      operation_identity | c.Blake3,
    },
  edges | Array { source | c.NonEmpty, target | c.NonEmpty },
  controls
    | {
      cancel | Bool,
      pause | std.contract.from_predicate (fun value => value == false),
      resume | std.contract.from_predicate (fun value => value == false),
    },
  bounds
    | {
      max_nodes | c.BoundedInteger 1 4294967295,
      max_edges | c.BoundedInteger 1 4294967295,
      max_log_references | c.BoundedInteger 1 4294967295,
      max_log_read_bytes | c.Positive,
      max_total_log_bytes | c.Positive,
    },
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
let FeedRefresh =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && (value == 0 || (value >= 60000 && value <= 86400000))
  )
in
let FeedPolicy = {
  token_file | c.AbsolutePath,
  max_subscriptions | c.BoundedInteger 1 128,
  max_entries_per_feed | c.BoundedInteger 1 200,
  max_feed_bytes | c.BoundedInteger 4096 4194304,
  max_redirects | c.BoundedInteger 0 5,
  timeout_seconds | c.BoundedInteger 1 30,
  refresh_interval_ms | FeedRefresh | default = 900000,
  allow_private_destinations | Bool | default = false,
}
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
let ProjectId =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 100
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let ProjectLabel =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 256
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in
let Reference =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1024
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in
let AdmittedReferences =
  std.contract.from_predicate (fun values =>
    let check = fun state value =>
      if std.array.elem value state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [value], duplicate = false }
    in
    std.array.length values <= 64
    && std.array.all
      (fun value =>
        std.string.length value > 0
        && std.string.length value <= 1024
        && !(std.string.is_match "^-" value)
        && !(std.string.is_match "[\\x00-\\x20\\x7f]" value)
      )
      values
    && !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
  )
in
let Unique = fun values =>
  std.array.length values == std.array.length (
    std.array.fold_left
      (fun seen value => if std.array.elem value seen then seen else seen @ [value])
      []
      values
  )
in
let ProfileRelations =
  std.contract.from_predicate (fun settings =>
    let bounds = settings.bounds in
    std.array.length settings.projects >= 1
    && std.array.length settings.projects <= bounds.max_projects
    && Unique (std.array.map (fun project => project.id) settings.projects)
    && settings.operator_token_file != settings.worker_token_file
    && (
      !(std.record.has_field "feeds" settings)
      || (
        settings.feeds.token_file != settings.operator_token_file
        && settings.feeds.token_file != settings.worker_token_file
      )
    )
    && std.array.all
      (fun project => project.workflow.bounds.max_log_read_bytes <= bounds.max_log_read_bytes)
      settings.projects
    && bounds.max_requests >= bounds.max_runs
    && bounds.max_artifact_bytes <= bounds.max_run_artifact_bytes
    && bounds.max_run_artifact_bytes <= bounds.max_total_artifact_bytes
    && bounds.max_artifact_bytes
    * 4
    / 3
    + 4096 <= bounds.max_request_bytes
  )
in
let Project = {
  id | ProjectId,
  label | ProjectLabel,
  repository_path | c.AbsolutePath,
  reference | Reference,
  admitted_references | Array c.NonEmpty | AdmittedReferences | default = [],
  source_limits | { max_entries | c.Positive, max_path_bytes | c.Positive, max_file_bytes | c.Positive, max_total_bytes | c.Positive },
  workflow | Workflow | WorkflowRelations,
  scope | Scope | optional,
  # r[impl kiln.untrusted.execution] Operator declaration that this project's
  # execution target runs each job in a microVM without credentials or cache;
  # only then are approved untrusted pull-request runs dispatched. Kiln cannot
  # verify the target's isolation. Omitted means not untrusted-capable.
  untrusted_capable | Bool | optional,
}
in
fun input =>
  let settings =
    input
      | {
        service_id | Identifier,
        listen_address | ListenAddress,
        origin | std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^https://[^/?#@ ]+$" value),
        tls | { certificate_path | c.AbsolutePath, private_key_path | c.AbsolutePath, min_version | std.contract.from_predicate (fun value => value == "TLS1.3") },
        operator_token_file | c.AbsolutePath,
        worker_token_file | c.AbsolutePath,
        state_directory | c.AbsolutePath,
        feeds | FeedPolicy | optional,
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
      | ProfileRelations
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
  & (if std.record.has_field "feeds" settings then { feeds = settings.feeds } else {})
