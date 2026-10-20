# r[impl kiln.observe.iroh.profile]
# r[impl kiln.observe.iroh.nonclaims]
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
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let BindAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^([0-9.]+|\\[[0-9a-fA-F:]+\\]):[0-9]+$" value
  )
in
let Peer =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Relay =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^https://[^\\x00]+$" value
  )
in
let Cohort = {
  iroh_version | Exact "0.95.1",
  iroh_blobs | Exact "deferred",
  bao_tree | Exact "deferred",
  alpn | Exact "kiln.observe.v1",
}
in
let Endpoint = {
  bind_address | BindAddress,
  relay_urls | Array Relay,
  max_connections | Positive,
  max_frame_bytes | Positive,
  max_payload_bytes | Positive,
  request_timeout_milliseconds | Positive,
  write_timeout_milliseconds | Positive,
}
in
let Peers = {
  allowed | Array Peer,
  revoked | Array Peer,
  max_peers | Positive,
}
in
let Profile = {
  schema_version | Exact 1,
  profile_id | NonEmptyString,
  cohort | Cohort,
  endpoint | Endpoint,
  peers | Peers,
  non_claims | Array NonEmptyString,
  ..
}
in
let MaxConnections = 16 in
{
  profile | Profile = {
    schema_version = 1,
    profile_id = "iroh-observation-v1",
    cohort = {
      iroh_version = "0.95.1",
      iroh_blobs = "deferred",
      bao_tree = "deferred",
      alpn = "kiln.observe.v1",
    },
    endpoint = {
      bind_address = "127.0.0.1:0",
      relay_urls = [],
      max_connections = MaxConnections,
      max_frame_bytes = 4194304,
      max_payload_bytes = 4194304,
      request_timeout_milliseconds = 5000,
      write_timeout_milliseconds = 5000,
    },
    peers = {
      allowed = [
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      ],
      revoked = [],
      max_peers = 64,
    },
    core = Core.profile,
    non_claims = [
      "iroh observation does not prove transport acceptance",
      "iroh observation does not prove live completeness",
      "verified payload delivery does not prove source truth",
      "iroh observation does not prove control execution",
      "iroh observation does not prove provider correctness",
      "iroh observation does not prove CI success",
      "iroh observation does not establish release readiness",
    ],
  },
}
