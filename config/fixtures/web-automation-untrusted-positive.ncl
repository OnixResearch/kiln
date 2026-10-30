# Approve holds untrusted pull requests until an operator approves the exact
# commit and may count their results for required CI; explicit deny keeps the
# fork refusal of profiles that omit the policy.
let make = import "../web-automation.ncl" in
make {
  max_records = 100,
  status_limit = 10,
  max_event_bytes = 65536,
  max_tick_effects = 2,
  triggers = [
    {
      id = "github-untrusted-fixture",
      project_id = "fixture",
      reference = "refs/heads/main",
      repository = { owner = "fixture", repository = "source", repository_id = 42, allow_fork_pull_requests = false, untrusted_pull_requests = "approve" },
      secret_file = "/isolated-fixture/github.secret",
      enabled = false,
      push = true,
      pull_request = true,
      pull_request_base = "main",
      untrusted_results_satisfy_required_ci = true,
    },
    {
      id = "github-deny-fixture",
      project_id = "fixture",
      reference = "refs/heads/main",
      repository = { owner = "fixture", repository = "source", repository_id = 42, allow_fork_pull_requests = false, untrusted_pull_requests = "deny" },
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
