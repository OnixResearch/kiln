# Optional read-only execution artifact route for observed implementation-provider graphs and logs.
let c = import "github-contracts.ncl" in
let BoundedInteger = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= minimum
    && value <= maximum
    && std.number.floor value == value
  )
in
let ProviderToken =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 64
    && std.string.is_match "^[A-Za-z0-9._-]+$" value
    && value != "."
    && value != ".."
  )
in
let AdmittedRoute = {
  provider_namespace | ProviderToken,
  native_kind | ProviderToken,
  artifact_directory | c.AbsolutePath,
  supported | Bool | default = true,
  supports_graph | Bool | default = true,
  supports_logs | Bool | default = true,
}
in
let Bounds = {
  max_artifact_bytes | BoundedInteger 1 2097152 | default = 524288,
  max_graph_nodes | BoundedInteger 1 256 | default = 128,
  max_graph_edges | BoundedInteger 1 1024 | default = 512,
  max_log_entries | BoundedInteger 1 512 | default = 128,
  max_log_bytes | BoundedInteger 1 65536 | default = 16384,
  freshness_window_ms | BoundedInteger 1 86400000 | default = 300000,
}
in
fun input =>
  let admitted =
    input
      | {
        implementation_database | c.AbsolutePath,
        routes | Array AdmittedRoute,
        bounds | Bounds | default = {},
      }
  in
  admitted & { schema = "kiln.execution-artifacts-profile.v1" }
