# r[impl kiln.observe.http.profile]
# r[impl kiln.observe.http.nonclaims]
let Core = import "observation-profile.ncl" in
let NonEmptyString =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
  )
in
let NonNegative =
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 0
  )
in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^/" value
  )
in
let TlsVersion =
  std.contract.from_predicate (fun value =>
    std.is_string value && (value == "tls12" || value == "tls13")
  )
in
let Principal =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && (
      std.string.is_match "^[0-9a-f]{64}$" value
      || std.string.is_match "^did:key:z[1-9A-HJ-NP-Za-km-z]+$" value
    )
  )
in
let Origin = {
  scheme | Exact "https",
  host | NonEmptyString,
  port | Positive,
}
in
let Tls = {
  certificate_path | AbsolutePath,
  private_key_path | AbsolutePath,
  min_version | TlsVersion,
}
in
let Auth = {
  kind | Exact "ed25519_request_v1",
  principal | Principal,
  max_clock_skew_seconds | NonNegative,
  replay_window_seconds | Positive,
  max_replay_entries | Positive,
  max_nonce_bytes | Positive,
}
in
let Content = {
  view_content_type | Exact "application/json",
  event_content_type | Exact "text/event-stream",
  canonical_json | Exact true,
  max_view_bytes | Positive,
  default_page_limit | Positive,
  default_log_bytes | Positive,
}
in
let Live = {
  max_records | Positive,
  max_dropped | Positive,
  poll_interval_milliseconds | Positive,
  max_stream_milliseconds | Positive,
  max_sse_events | Positive,
}
in
let Bounds = {
  max_request_bytes | Positive,
  max_header_bytes | Positive,
  max_header_fields | Positive,
  max_connections | Positive,
  read_timeout_milliseconds | Positive,
  write_timeout_milliseconds | Positive,
  max_error_bytes | Positive,
}
in
let Profile = {
  schema_version | Exact 1,
  profile_id | NonEmptyString,
  origin | Origin,
  tls | Tls,
  auth | Auth,
  content | Content,
  live | Live,
  bounds | Bounds,
  non_claims | Array NonEmptyString,
  ..
}
in
let MaxConnections = 16 in
{
  profile | Profile = {
    schema_version = 1,
    profile_id = "http-observation-v1",
    origin = {
      scheme = "https",
      host = "observe.example.invalid",
      port = 443,
    },
    tls = {
      certificate_path = "/var/lib/kiln/observe/tls/chain.pem",
      private_key_path = "/var/lib/kiln/observe/tls/key.pem",
      min_version = "tls12",
    },
    auth = {
      kind = "ed25519_request_v1",
      principal = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      max_clock_skew_seconds = 60,
      replay_window_seconds = 300,
      max_replay_entries = 1024,
      max_nonce_bytes = 64,
    },
    content = {
      view_content_type = "application/json",
      event_content_type = "text/event-stream",
      canonical_json = true,
      max_view_bytes = 4194304,
      default_page_limit = 2,
      default_log_bytes = 4,
    },
    live = {
      max_records = 4,
      max_dropped = 64,
      poll_interval_milliseconds = 25,
      max_stream_milliseconds = 60000,
      max_sse_events = 256,
    },
    bounds = {
      max_request_bytes = 16384,
      max_header_bytes = 8192,
      max_header_fields = 64,
      max_connections = MaxConnections,
      read_timeout_milliseconds = 5000,
      write_timeout_milliseconds = 5000,
      max_error_bytes = 4096,
    },
    core = Core.profile,
    non_claims = [
      "http observation does not prove transport acceptance",
      "http observation does not prove live completeness",
      "http observation does not prove control execution",
      "http observation does not prove provider correctness",
      "http observation does not prove CI success",
      "http observation does not establish release readiness",
    ],
  },
}
