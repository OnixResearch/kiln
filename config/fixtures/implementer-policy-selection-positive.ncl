# Reviewed pilot intake policy that admits exactly issue #1; every other open
# issue in the repository is observed as unselected and never admitted.
let input = {
  owner = "OnixResearch",
  repository = "implementer-sandbox",
  repository_id = 42,
  project = "implementer-sandbox",
  requested_outcome = "implement the admitted issue",
  acceptance = [
    { id = "accepted", statement = "the exact admitted work passes its verification profile" },
  ],
  dependencies = [],
  permitted_paths = ["src"],
  permitted_effects = ["write_source"],
  verification_profile = "1111111111111111111111111111111111111111111111111111111111111111",
  attempts = 1,
  minutes_per_attempt = 20,
  terminal_policy = "verified_pull_request",
  priority = 10,
  selection = { issues = [1] },
}
in
(import "../implementer-policy.ncl") input
