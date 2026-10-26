# Isolated conformance inputs only; these paths and handler identities are not deployments.
{
  service_id = "fixture-web",
  listen_address = "127.0.0.1:9443",
  origin = "https://localhost:9443",
  tls = { certificate_path = "/tmp/kiln-web-fixture/certificate.pem", private_key_path = "/tmp/kiln-web-fixture/key.pem", min_version = "TLS1.3" },
  operator_token_file = "/tmp/kiln-web-fixture/operator.token",
  worker_token_file = "/tmp/kiln-web-fixture/worker.token",
  state_directory = "/tmp/kiln-web-fixture/state",
  projects = [
    {
      id = "fixture",
      label = "Isolated fixture",
      repository_path = "/tmp/kiln-web-fixture/repository",
      reference = "refs/heads/main",
      source_limits = { max_entries = 100, max_path_bytes = 1024, max_file_bytes = 1048576, max_total_bytes = 4194304 },
      workflow = {
        socket_path = "/tmp/kiln-web-fixture/lattice.sock",
        allowed_user_ids = [1000],
        transport_limits = { maximum_frame_bytes = 1048576, maximum_log_bytes = 65536, maximum_connections = 4, read_deadline_milliseconds = 3000, write_deadline_milliseconds = 3000 },
        adapter_limits = { maximum_request_bytes = 1048576, maximum_response_bytes = 1048576 },
        policy_id = "fixture-policy",
        provider_profile_id = "fixture-lattice",
        platform = "linux-x86_64",
        nodes = [{ id = "job", kind = "test", effect_class = "test", adapter_kind = "local", operation_identity = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }],
        edges = [],
        controls = { cancel = true, pause = false, resume = false },
        bounds = { max_nodes = 8, max_edges = 16, max_log_references = 32, max_log_read_bytes = 65536, max_total_log_bytes = 4194304 },
      },
    }
  ],
  bounds = { max_request_bytes = 2097152, max_response_bytes = 4194304, max_header_bytes = 16384, max_connections = 8, timeout_seconds = 10, poll_interval_ms = 100, max_projects = 4, max_runs = 16, max_artifact_bytes = 1048576, max_artifacts = 8, max_run_artifact_bytes = 4194304, max_total_artifact_bytes = 16777216, max_log_read_bytes = 65536, max_requests = 128, max_events_per_run = 8, max_state_bytes = 16777216 },
}
