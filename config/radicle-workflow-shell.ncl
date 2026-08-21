# r[impl kiln.radicle.protocol] r[impl kiln.radicle.nonclaims]
let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let AbsolutePath = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^/[^\\x00]+$" value
) in
let Blake3Hex = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let RuntimeMode = std.contract.from_predicate (fun value =>
  value == "production"
) in
let TriggerOnly = std.contract.from_predicate (fun value =>
  value == {
    source = false,
    trigger = true,
    status = false,
    collaboration = false,
  }
) in
let EffectClass = std.contract.from_predicate (fun value =>
  value == "build" || value == "test"
) in
let Node = {
  id | NonEmpty,
  kind | NonEmpty,
  effect_class | EffectClass,
  adapter_kind | NonEmpty,
  operation_identity | Blake3Hex,
} in
let Edge = {
  source | NonEmpty,
  target | NonEmpty,
} in
let Profile = {
  schema | std.contract.from_predicate (fun value => value == "kiln.radicle-workflow-shell.v1"),
  mode | RuntimeMode,
  radicle_runtime_revision | NonEmpty,
  radicle_broker_revision | NonEmpty,
  radicle_license | NonEmpty,
  lattice_source | NonEmpty,
  lattice_revision | NonEmpty,
  lattice_license | NonEmpty,
  capabilities | TriggerOnly,
  socket_path | AbsolutePath,
  replay_database_path | AbsolutePath,
  maximum_replay_records | Positive,
  maximum_polls | Positive,
  allowed_user_ids | Array Number,
  transport_limits | {
    maximum_frame_bytes | Positive,
    maximum_log_bytes | Positive,
    maximum_connections | Positive,
    read_deadline_milliseconds | Positive,
    write_deadline_milliseconds | Positive,
  },
  adapter_limits | {
    max_request_bytes | Positive,
    max_response_bytes | Positive,
    max_in_flight | Positive,
  },
  workflow | {
    policy_id | NonEmpty,
    provider_profile_id | NonEmpty,
    platform | NonEmpty,
    nodes | Array Node,
    edges | Array Edge,
    controls | { cancel | Bool, pause | Bool, resume | Bool },
    bounds | {
      max_nodes | Positive,
      max_edges | Positive,
      max_log_references | Positive,
      max_log_read_bytes | Positive,
      max_total_log_bytes | Positive,
    },
  },
  non_claims | Array NonEmpty,
} in
let FrameBytes = 33554432 in
let LogBytes = 16777216 in
let Connections = 64 in
let DeadlineMilliseconds = 30000 in
let ReplayRecords = 4096 in
let Polls = 64 in
let UserId = 1000 in
let MaxNodes = 256 in
let MaxEdges = 1024 in
let MaxLogReferences = 256 in
let MaxLogReadBytes = 1048576 in
let MaxTotalLogBytes = 16777216 in
let MaxRequestBytes = 1048576 in
let MaxResponseBytes = 16777216 in
let MaxInFlight = 64 in
let OperationIdentity = "1111111111111111111111111111111111111111111111111111111111111111" in
{
  profile | Profile = {
    schema = "kiln.radicle-workflow-shell.v1",
    mode = "production",
    radicle_runtime_revision = "radicle-0.24.0",
    radicle_broker_revision = "a688afde1f730ef60402a809d9cdadc55e8dd749",
    radicle_license = "MIT OR Apache-2.0",
    lattice_source = "ssh://git@github.com/OnixResearch/lattice.git",
    lattice_revision = "70496e67c7fd4a8b05914161a8e09de2759bebc8",
    lattice_license = "MIT",
    capabilities = {
      source = false,
      trigger = true,
      status = false,
      collaboration = false,
    },
    socket_path = "/run/kiln/lattice.sock",
    replay_database_path = "/var/lib/kiln/radicle-replay.sqlite",
    maximum_replay_records = ReplayRecords,
    maximum_polls = Polls,
    allowed_user_ids = [UserId],
    transport_limits = {
      maximum_frame_bytes = FrameBytes,
      maximum_log_bytes = LogBytes,
      maximum_connections = Connections,
      read_deadline_milliseconds = DeadlineMilliseconds,
      write_deadline_milliseconds = DeadlineMilliseconds,
    },
    adapter_limits = {
      max_request_bytes = MaxRequestBytes,
      max_response_bytes = MaxResponseBytes,
      max_in_flight = MaxInFlight,
    },
    workflow = {
      policy_id = "radicle-ci-workflow-v1",
      provider_profile_id = "lattice-v1",
      platform = "linux-x86_64",
      nodes = [{
        id = "build",
        kind = "build",
        effect_class = "build",
        adapter_kind = "local",
        operation_identity = OperationIdentity,
      }],
      edges = [],
      controls = { cancel = true, pause = false, resume = false },
      bounds = {
        max_nodes = MaxNodes,
        max_edges = MaxEdges,
        max_log_references = MaxLogReferences,
        max_log_read_bytes = MaxLogReadBytes,
        max_total_log_bytes = MaxTotalLogBytes,
      },
    },
    non_claims = [
      "broker admission does not prove event completeness",
      "source context does not prove source trust or revision availability",
      "workflow acceptance does not prove an external effect started",
      "workflow observation does not replace canonical Kiln CI state",
      "local replay admission does not prove distributed exactly-once delivery",
      "adapter checks do not establish release readiness",
    ],
  },
}
