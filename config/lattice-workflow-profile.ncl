# r[impl kiln.lattice.profile] r[impl kiln.lattice.nonclaims]
let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let ExactSource = std.contract.from_predicate (fun value =>
  value == "ssh://git@github.com/OnixResearch/lattice.git"
) in
let ExactRevision = std.contract.from_predicate (fun value =>
  value == "66fe7b9b43567490caacc797ebda307e734c4419"
) in
let ExactLicense = std.contract.from_predicate (fun value => value == "MIT") in
let RequestSchema = std.contract.from_predicate (fun value =>
  value == "lattice.workflow-request.v1"
) in
let ObservationSchema = std.contract.from_predicate (fun value =>
  value == "lattice.workflow-observation.v1"
) in
let GraphSchema = std.contract.from_predicate (fun value =>
  value == "lattice.workflow-graph.v1"
) in
let Platform = std.contract.from_predicate (fun value => value == "linux-x86_64") in
let RequiredNodes = std.contract.from_predicate (fun value => value == ["build", "test"]) in
let RequiredEffects = std.contract.from_predicate (fun value => value == ["build", "test"]) in
let RequiredAdapters = std.contract.from_predicate (fun value => value == ["local", "remote"]) in
let RequiredControls = std.contract.from_predicate (fun value =>
  value == { cancel = true, pause = true, resume = true }
) in
let Profile = {
  source | ExactSource,
  revision | ExactRevision,
  license | ExactLicense,
  request_schema | RequestSchema,
  observation_schema | ObservationSchema,
  graph_schema | GraphSchema,
  endpoint_profile | NonEmpty,
  platform | Platform,
  node_kinds | RequiredNodes,
  effect_classes | RequiredEffects,
  adapter_kinds | RequiredAdapters,
  controls | RequiredControls,
  observation_mode | NonEmpty,
  limits | {
    max_nodes | Positive,
    max_edges | Positive,
    max_log_references | Positive,
    max_log_read_bytes | Positive,
    max_total_log_bytes | Positive,
    max_request_bytes | Positive,
    max_response_bytes | Positive,
    max_in_flight | Positive,
    timeout_milliseconds | Positive,
  },
  non_claims | Array NonEmpty,
} in
let MaxNodes = 256 in
let MaxEdges = 1024 in
let MaxLogReferences = 256 in
let MaxLogReadBytes = 1048576 in
let MaxTotalLogBytes = 16777216 in
let MaxRequestBytes = 1048576 in
let MaxResponseBytes = 16777216 in
let MaxInFlight = 64 in
let TimeoutMilliseconds = 30000 in
{
  profile | Profile = {
    source = "ssh://git@github.com/OnixResearch/lattice.git",
    revision = "66fe7b9b43567490caacc797ebda307e734c4419",
    license = "MIT",
    request_schema = "lattice.workflow-request.v1",
    observation_schema = "lattice.workflow-observation.v1",
    graph_schema = "lattice.workflow-graph.v1",
    endpoint_profile = "lattice-local-v1",
    platform = "linux-x86_64",
    node_kinds = ["build", "test"],
    effect_classes = ["build", "test"],
    adapter_kinds = ["local", "remote"],
    controls = { cancel = true, pause = true, resume = true },
    observation_mode = "poll",
    limits = {
      max_nodes = MaxNodes,
      max_edges = MaxEdges,
      max_log_references = MaxLogReferences,
      max_log_read_bytes = MaxLogReadBytes,
      max_total_log_bytes = MaxTotalLogBytes,
      max_request_bytes = MaxRequestBytes,
      max_response_bytes = MaxResponseBytes,
      max_in_flight = MaxInFlight,
      timeout_milliseconds = TimeoutMilliseconds,
    },
    non_claims = [
      "Lattice admission does not prove workflow correctness",
      "Lattice observations do not replace canonical Kiln CI state",
      "Lattice checks do not prove host confinement or external effect success",
      "Lattice checks do not prove credential freshness or release readiness",
    ],
  },
}
