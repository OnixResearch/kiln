# r[impl kiln.dashboard.repositories.delivery]
# Human configuration is admitted here; export the applied function as JSON for
# scripts/dashboard-publish.py --config CONFIG.json [--watch]. Paths other than
# destination.path resolve against CONFIG.json's directory. Both report parents
# must already exist; this shell never creates remote directories or services.
let MaximumReportBytes = 67108864 in
let MaximumTimeoutSeconds = 3600 in
let MaximumIntervalMilliseconds = 86400000 in
let MaximumPathCharacters = 4096 in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Integer = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= maximum && std.number.floor value == value
  )
in
let Path =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= MaximumPathCharacters
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/[^\\x00-\\x1f\\x7f]*$" value
    && !std.string.is_match "(/$|/\\.$|/\\.\\.$)" value
    && std.string.length value <= MaximumPathCharacters
  )
in
let SSHHost =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 253
    && std.string.is_match "^([A-Za-z0-9_][A-Za-z0-9_.-]*@)?[A-Za-z0-9_][A-Za-z0-9_.-]*$" value
  )
in
let LocalDestination = { kind | Exact "local", path | AbsolutePath } in
let SSHDestination = { kind | Exact "ssh", host | SSHHost, path | AbsolutePath } in
let Destination =
  std.contract.custom (fun label value =>
    if std.is_record value && std.record.has_field "kind" value then
      if value.kind == "local" then
        std.contract.check LocalDestination label value
      else if value.kind == "ssh" then
        std.contract.check SSHDestination label value
      else
        'Error { message = "unsupported destination kind" }
    else
      'Error { message = "destination requires a kind" }
  )
in
let Configuration = {
  schema_version | Exact 1,
  dashboard_executable | Path,
  catalog | Path,
  profile | Path,
  local_report | Path,
  destination | Destination,
  max_report_bytes | Integer MaximumReportBytes,
  subprocess_timeout_seconds | Integer MaximumTimeoutSeconds,
  interval_ms | Integer MaximumIntervalMilliseconds,
}
in
# No executable, source paths, SSH identity or publication target is invented.
fun settings => settings | Configuration
