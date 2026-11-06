# Fixed native Lattice shell task. Hosted remains the execution backend.
let c = import "github-contracts.ncl" in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let Digest =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Origin =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 2048
    && std.string.is_match "^https://[^/?#@ ]+$" value
  )
in
let Window = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0 && value <= maximum && value == std.number.floor value
  )
in
let SocketAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^([0-9.]+|\\[[0-9a-fA-F:]+\\]):[0-9]+$" value
  )
in
let Https =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 2048
    && std.string.is_match "^https://[^ @?#]+$" value
  )
in
let Address =
  std.contract.custom (fun label value =>
    if std.record.has_field "Ip" value then
      std.contract.check { Ip | SocketAddress } label value
    else if std.record.has_field "Relay" value then
      std.contract.check { Relay | Https } label value
    else
      'Error { message = "Endpoint transport address must be Ip or Relay" }
  )
in
fun input =>
  let lookup_window = if std.record.has_field "lookup_timeout_ms" input then input.lookup_timeout_ms else 10000 in
  input
    | {
      schema
        | std.contract.from_predicate (fun value => value == "kiln.hosted.lattice-job.v1")
        | default
        = "kiln.hosted.lattice-job.v1",
      web
        | {
          service_origin | Origin,
          tls_ca_path | c.AbsolutePath,
          token_file | c.AbsolutePath,
          http_timeout_ms | Window 86400000,
          maximum_response_bytes | Window 1048576,
        },
      network
        | {
          key_file | c.AbsolutePath,
          bind_address | SocketAddress,
          relay_urls | Array Https | std.contract.from_predicate (fun value => std.array.length value <= 8),
        },
      remote
        | {
          id | Digest,
          addrs
            | Array Address
            | std.contract.from_predicate (fun value =>
              std.array.length value > 0 && std.array.length value <= 16
            ),
        },
      organization | Identifier,
      project_id | Identifier,
      repository_id | Digest,
      node_id | Identifier,
      policy_id | Identifier,
      lookup_timeout_ms | Window 60000 | default = 10000,
      poll_interval_ms
        | std.contract.from_predicate (fun value =>
          std.is_number value
          && value >= 10
          && value <= 5000
          && value == std.number.floor value
          && value <= lookup_window
        )
        | default
        = 100,
      maximum_run_ms | Window 86400000 | default = 120000,
      cleanup_timeout_ms
        | std.contract.from_predicate (fun value =>
          std.is_number value
          && value >= 1000
          && value <= 120000
          && value == std.number.floor value
        )
        | default
        = 10000,
    }
