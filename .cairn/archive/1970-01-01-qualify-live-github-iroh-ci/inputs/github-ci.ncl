# Exact isolated live controller input. Secrets remain in a private real provider.
let make = import "../../../../config/github-ci.ncl" in
let Secret = {
  program_path = "/nix/store/jxryiy3mpskq9zl0qxm6zjknsbammcqm-secretspec-0.19.0/bin/secretspec",
  program_blake3 = "486aab8bc242c6b8a9b724f7cdd0c26160f2b4edb32cc20ff1b66799cb344791",
  project_root = "/var/tmp/kiln-live-ci-20261018/secrets",
  profile = "default",
  provider = "local",
  secret_key = "WEBHOOK_SECRET",
} in
make {
  owner = "onixresearch",
  repository = "kiln",
  repository_id = 1421187104,
  replay_database_path = "/var/tmp/kiln-live-ci-20261018/github-shell.sqlite",
  webhook_secret = Secret,
  status_secret = Secret & { secret_key | force = "STATUS_TOKEN" },
  workflow = {
    socket_path = "/var/tmp/kiln-live-ci-20261018/iroh.sock",
    allowed_user_ids = [965],
    transport_limits = {
      maximum_frame_bytes = 33554432,
      maximum_log_bytes = 16777216,
      maximum_connections = 16,
      read_deadline_milliseconds = 30000,
      write_deadline_milliseconds = 30000,
    },
    adapter_limits = {
      maximum_request_bytes = 1048576,
      maximum_response_bytes = 16777216,
    },
    policy_id = "github-live-20261018",
    provider_profile_id = "lattice-v1",
    platform = "linux-x86_64",
    nodes = [{
      id = "probe",
      kind = "test",
      effect_class = "test",
      adapter_kind = "local",
      operation_identity = "eb669c2693415d283051ba8e5fcc1d02d4d846a7412099c2f0ed4f29357713dc",
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
