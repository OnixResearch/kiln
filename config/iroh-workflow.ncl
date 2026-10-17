# r[impl kiln.iroh.workflow.profile] r[impl kiln.iroh.workflow.peers]
let MaximumPeers = 64 in
let MaximumRelays = 8 in
let MaximumAllowedUsers = 64 in
let MaximumConcurrentExchanges = 64 in
let MaximumFrameBytes = 33554432 in
let MaximumLogBytes = 16777216 in
let MaximumDeadlineMilliseconds = 300000 in
let MaximumUserId = 4294967295 in
let MaximumCapabilityItems = 256 in
let MaximumLabelBytes = 128 in
let MaximumNodes = 4096 in
let MaximumEdges = 16384 in
let MaximumLogReferences = 4096 in
let MaximumLogReadBytes = 16777216 in
let MaximumTotalLogBytes = 268435456 in
let Integer = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= minimum
    && value <= maximum
    && std.number.floor value == value
  )
in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let AbsoluteSocketPath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/[^\\x00]+$" value
    && !std.string.is_match "(^|/)\\.{1,2}(/|$)|/$" value
  )
in
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
let BoundedArray = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= minimum && std.array.length value <= maximum
  )
in
let OrderedUserIds =
  std.contract.from_predicate (fun values =>
    (
      std.array.fold_left
        (fun state item =>
          {
            previous = item,
            first = false,
            valid = state.valid && (state.first || state.previous < item),
          }
        )
        { previous = 0, first = true, valid = true }
        values
    ).valid
  )
in
let Label =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= MaximumLabelBytes
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let EffectClass =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["read", "build", "test", "publish", "notify", "custom"]
  )
in
let WorkflowNonClaims = [
  "workflow admission does not prove workflow correctness",
  "workflow admission does not prove host confinement",
  "workflow observation does not prove an external effect succeeded",
  "workflow observation does not prove credential freshness",
  "workflow observation does not establish release readiness",
]
in
let TransportNonClaims = [
  "transport admission does not prove workflow correctness",
  "transport peer identity does not prove peer intent",
  "transport acceptance does not prove an operation executed",
  "transport response does not prove an external effect succeeded",
  "transport checks do not establish production deployment",
]
in
let NativeContract = {
  schema | Exact "lattice.workflow-profile.v1",
  graph_schema | Exact "lattice.workflow-graph.v1",
  platforms | Array Label | BoundedArray 1 MaximumCapabilityItems,
  node_kinds | Array Label | BoundedArray 1 MaximumCapabilityItems,
  effect_classes | Array EffectClass | BoundedArray 1 MaximumCapabilityItems,
  adapter_kinds | Array Label | BoundedArray 1 MaximumCapabilityItems,
  controls | { cancel | Bool, pause | Bool, resume | Bool },
  limits
    | {
      maximum_nodes | Integer 1 MaximumNodes,
      maximum_edges | Integer 1 MaximumEdges,
      maximum_log_references | Integer 1 MaximumLogReferences,
      maximum_log_read_bytes | Integer 1 MaximumLogReadBytes,
      maximum_total_log_bytes | Integer 1 MaximumTotalLogBytes,
    },
  non_claims | Exact WorkflowNonClaims,
}
in
let TransportLimits = {
  maximum_frame_bytes | Integer 1 MaximumFrameBytes,
  maximum_log_bytes | Integer 1 MaximumLogBytes,
  maximum_connections | Integer 1 MaximumConcurrentExchanges,
  read_deadline_milliseconds | Integer 1 MaximumDeadlineMilliseconds,
  write_deadline_milliseconds | Integer 1 MaximumDeadlineMilliseconds,
}
in
let Settings = {
  schema | Exact "kiln.iroh-workflow-profile.v1",
  iroh_version | Exact "0.95.1",
  alpn | Exact "kiln.workflow.v1",
  socket_path | AbsoluteSocketPath,
  bind_address | BindAddress,
  allowed_peers | Array Peer | BoundedArray 1 MaximumPeers,
  allowed_user_ids | Array (Integer 0 MaximumUserId) | BoundedArray 1 MaximumAllowedUsers | OrderedUserIds,
  relay_urls | Array Relay | BoundedArray 0 MaximumRelays,
  transport_limits | TransportLimits,
  contract | NativeContract,
  exchange_timeout_ms | Integer 1 MaximumDeadlineMilliseconds,
}
in
# No endpoints, enrollment, relays or credentials are invented here. The operator
# supplies every setting. Empty relay_urls disables relays; there is no discovery.
fun settings =>
  let checked | Settings = settings in
  {
    schema = checked.schema,
    iroh_version = checked.iroh_version,
    alpn = checked.alpn,
    socket_path = checked.socket_path,
    bind_address = checked.bind_address,
    allowed_peers = checked.allowed_peers,
    relay_urls = checked.relay_urls,
    transport = {
      schema = "lattice.workflow-transport-profile.v1",
      limits = checked.transport_limits,
      peers = { allowed_user_ids = checked.allowed_user_ids },
      non_claims = TransportNonClaims,
    },
    contract = checked.contract,
    exchange_timeout_ms = checked.exchange_timeout_ms,
  }
