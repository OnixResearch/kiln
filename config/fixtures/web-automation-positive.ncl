let make = import "../web-automation.ncl" in
make {
  max_records = 100,
  status_limit = 10,
  max_event_bytes = 65536,
  max_tick_effects = 2,
  triggers = [
    {
      id = "github-fixture",
      project_id = "fixture",
      reference = "refs/heads/main",
      repository = { owner = "fixture", repository = "source", repository_id = 42, allow_fork_pull_requests = false },
      secret_file = "/isolated-fixture/github.secret",
      enabled = false,
      push = true,
      pull_request = true,
      pull_request_base = "main",
    }
  ],
  schedules = [
    {
      id = "periodic-fixture",
      project_id = "fixture",
      reference = "refs/heads/main",
      enabled = false,
      interval_ms = 60000,
      min_interval_ms = 1000,
      max_interval_ms = 3600000,
      next_due_ms = null,
    }
  ],
  notifications = [
    {
      id = "receiver-fixture",
      project_id = "fixture",
      enabled = false,
      on_success = true,
      on_failure = true,
      host = "localhost",
      port = 9444,
      path = "/completion",
      addresses = ["127.0.0.1"],
      bearer_secret_file = null,
      ca_certificate_file = "/isolated-fixture/receiver-ca.pem",
      allow_loopback = true,
      allow_private = false,
      timeout_ms = 5000,
      max_request_bytes = 65536,
      max_response_bytes = 65536,
      max_resolver_candidates = 4,
    }
  ],
}
