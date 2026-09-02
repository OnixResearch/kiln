# CONFORMANCE ONLY: synthetic IDs and paths, never a deployable service profile.
let make = import "../../config/github-ci.ncl" in
let Secret = {
  program_path = "/fixture/secretspec",
  program_blake3 = "1111111111111111111111111111111111111111111111111111111111111111",
  project_root = "/fixture/secrets",
  profile = "fixture",
  provider = null,
  secret_key = "WEBHOOK_SECRET",
} in
make {
  owner = "example",
  repository = "project",
  repository_id = 42,
  replay_database_path = "/fixture/github.sqlite",
  webhook_secret = Secret,
  status_secret = Secret & { secret_key | force = "STATUS_TOKEN" },
  workflow = {
    socket_path = "/fixture/lattice.sock",
    allowed_user_ids = [1000],
    transport_limits = {
      maximum_frame_bytes = 33554432,
      maximum_log_bytes = 16777216,
      maximum_connections = 16,
      read_deadline_milliseconds = 30000,
      write_deadline_milliseconds = 30000,
    },
    adapter_limits = { maximum_request_bytes = 1048576, maximum_response_bytes = 16777216 },
    policy_id = "github-ci-fixture",
    provider_profile_id = "lattice-v1",
    platform = "linux-x86_64",
    nodes = [{
      id = "test",
      kind = "test",
      effect_class = "test",
      adapter_kind = "local",
      operation_identity = "1111111111111111111111111111111111111111111111111111111111111111",
    }],
    edges = [],
    controls = { cancel = true, pause = false, resume = false },
    bounds = {
      max_nodes = 256,
      max_edges = 1024,
      max_log_references = 256,
      max_log_read_bytes = 1048576,
      max_total_log_bytes = 16777216,
    },
  },
}
