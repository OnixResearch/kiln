# Export once to JSON; pin BLAKE3 of the original exported bytes with --identity.
# Credential is an explicit private owned file, never an environment variable.
# Requests are HTTPS-only, trust only this CA, never redirect or retry mutations.
let Integer = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= maximum && std.number.floor value == value
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/[^\\x00-\\x1f\\x7f]+$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
fun input =>
  let settings =
    input
      | {
        service_origin
          | std.contract.from_predicate (fun value =>
            std.is_string value && std.string.is_match "^https://[^/?#@]+$" value
          ),
        token_file | AbsolutePath,
        tls_ca_path | AbsolutePath,
        http_timeout_ms | Integer 86400000 | default = 10000,
        poll_interval_ms | Integer 86400000 | default = 250,
        watch_timeout_ms | Integer 86400000 | default = 900000,
        maximum_request_bytes | Integer 16777216 | default = 1048576,
        maximum_response_bytes | Integer 16777216 | default = 1048576,
      }
  in
  {
    schema = "kiln.web-developer.v1",
    service_origin = settings.service_origin,
    token_file = settings.token_file,
    tls_ca_path = settings.tls_ca_path,
    http_timeout_ms = settings.http_timeout_ms,
    poll_interval_ms = settings.poll_interval_ms,
    watch_timeout_ms = settings.watch_timeout_ms,
    maximum_request_bytes = settings.maximum_request_bytes,
    maximum_response_bytes = settings.maximum_response_bytes,
  }
