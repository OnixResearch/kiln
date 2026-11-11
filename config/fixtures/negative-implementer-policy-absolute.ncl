# An absolute permitted path is not a repository-relative edit scope.
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
  permitted_paths = ["/etc/passwd"],
  permitted_effects = ["write_source"],
  verification_profile = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  attempts = 2,
  minutes_per_attempt = 60,
  terminal_policy = "verified_pull_request",
  priority = 10,
}
in
(import "../implementer-policy.ncl") input
