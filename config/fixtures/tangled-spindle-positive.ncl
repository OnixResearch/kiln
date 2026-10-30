# Fixture inputs only: example DIDs and paths, not a deployment.
(import "../tangled-spindle.ncl") {
  spindle_did = "did:web:spindle.example.com",
  listen_address = "127.0.0.1:6555",
  state_directory = "/var/lib/kiln/tangled",
  git_program = "/run/current-system/sw/bin/git",
  repositories = [
    {
      repo_did = "did:plc:abcdefghijklmnopqrstuvwx",
      knot_did = "did:web:knot.example.com",
      clone_url = "https://knot.example.com/did:plc:abcdefghijklmnopqrstuvwx",
      trusted_actors = ["did:plc:trustedactor234567abcdef"],
      approvers = ["did:plc:approver234567abcdefghij"],
    },
  ],
  engines = [
    {
      engine = "microvm",
      images = ["nixos"],
      packages = ["go", "nodejs"],
      platform = "linux-x86_64",
      adapter_kind = "microvm",
    },
  ],
  workflow = {
    policy_id = "tangled-trusted-v1",
    untrusted_policy_id = "tangled-untrusted-v1",
    provider_profile_id = "lattice-v1",
    controls = { cancel = true, pause = false, resume = false },
    bounds = {
      max_nodes = 4,
      max_edges = 4,
      max_log_references = 16,
      max_log_read_bytes = 65536,
      max_total_log_bytes = 1048576,
    },
  },
  status_endpoint = "https://status.example.com/kiln/tangled",
  status_secret_path = "/run/credentials/kiln-tangled/status-secret",
}
