# The retired fork switch cannot grant trusted fork execution, even with approve.
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
      repository = { owner = "fixture", repository = "source", repository_id = 42, allow_fork_pull_requests = true, untrusted_pull_requests = "approve" },
      secret_file = "/isolated-fixture/github.secret",
      enabled = false,
      push = true,
      pull_request = true,
      pull_request_base = "main",
    }
  ],
  schedules = [],
  notifications = [],
}
