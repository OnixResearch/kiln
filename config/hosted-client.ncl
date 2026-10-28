# Customer-local key and browser bridge. Remote uses pinned iroh 0.95.1
# EndpointAddr JSON: { id, addrs = [{ Ip = "IP:port" }, { Relay = "https://…" }] }.
# Copy the service's endpoint output; an endpoint id alone supplies no route.
let c = import "github-contracts.ncl" in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let Peer =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Https =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 2048
    && std.string.is_match "^https://[^ @?#]+$" value
  )
in
let SocketAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^([0-9.]+|\\[[0-9a-fA-F:]+\\]):[0-9]+$" value
  )
in
let RelayBound =
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= 8
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
let Loopback =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^(127\\.[0-9]+\\.[0-9]+\\.[0-9]+|\\[::1\\]):[0-9]+$" value
  )
in
fun input =>
  input
    | {
      schema
        | std.contract.from_predicate (fun value => value == "kiln.hosted.v1")
        | default
        = "kiln.hosted.v1",
      network
        | {
          key_file | c.AbsolutePath,
          bind_address | SocketAddress,
          relay_urls | Array Https | RelayBound,
        },
      remote | { id | Peer, addrs | Array Address },
      organization | Identifier,
      listen_address | Loopback,
      webhook
        | c.Nullable {
          project | Identifier,
          connection | Identifier,
          generation | c.Positive,
        }
        | default
        = null,
    }
