# runbook_routing with neither labels nor commands must be rejected.
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
  permitted_paths = ["crates/kiln-core"],
  permitted_effects = ["write_source"],
  verification_profile = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  attempts = 1,
  minutes_per_attempt = 20,
  terminal_policy = "verified_pull_request",
  priority = 10,
  runbook_routing = {
    allowed_actors = ["maintainer-a"],
    labels = [],
    commands = [],
  },
}
in
(import "../implementer-policy.ncl") input
