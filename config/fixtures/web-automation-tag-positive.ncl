# Tag-bound push with a tag glob filter.
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
      reference = "refs/tags/v1.0.0",
      repository = { owner = "fixture", repository = "source", repository_id = 42, allow_fork_pull_requests = false },
      secret_file = "/isolated-fixture/github.secret",
      enabled = false,
      push = true,
      pull_request = false,
      pull_request_base = null,
      filter = { tags = ["v*"] },
    }
  ],
  schedules = [],
  notifications = [],
}
