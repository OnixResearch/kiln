# Reviewed implementation policy with label and `/runbook <id>` routing.
let input = {
  owner = "OnixResearch",
  repository = "kiln",
  repository_id = 42,
  project = "kiln",
  requested_outcome = "implement the admitted issue",
  acceptance = [
    { id = "accepted", statement = "the exact admitted work passes its verification profile" },
  ],
  dependencies = [],
  permitted_paths = ["crates/kiln-core", "crates/kiln-web-service"],
  permitted_effects = ["write_source", "run_tests"],
  verification_profile = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  attempts = 2,
  minutes_per_attempt = 60,
  terminal_policy = "verified_pull_request",
  priority = 10,
  runbook_routing = {
    allowed_actors = ["maintainer-a", "maintainer-b"],
    allow_issue_comments = true,
    allow_pull_request_comments = true,
    require_push_association = true,
    console_base_url = "https://kiln.example.invalid",
    labels = [
      {
        label = "runbook:flake-update",
        runbook_id = "flake-update",
        target_ref = "refs/heads/main",
        params = { dry_run = false, depth = 1 },
        mode = 'dispatch,
      },
      {
        label = "runbook:fleet-check",
        runbook_id = "fleet-check",
        mode = 'fanout,
        fanout = {
          targets = ["OnixResearch/kiln@refs/heads/main"],
          max_concurrency = 2,
          budget_cap = 8,
          claim_policy = 'refuse,
          scope_path = "crates/kiln-core",
        },
      },
    ],
    commands = [
      {
        command = "flake-update",
        runbook_id = "flake-update",
        target = "OnixResearch/kiln@refs/heads/main",
        params = { dry_run = true },
        mode = 'dispatch,
      },
    ],
  },
}
in
(import "../implementer-policy.ncl") input
