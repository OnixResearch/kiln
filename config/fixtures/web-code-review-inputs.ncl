{
  database = "/var/lib/kiln/review/review.sqlite",
  project = "fixture",
  provider = {
    repository_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    repository_path = "/var/lib/kiln/source",
    target_refs = ["refs/heads/main"],
    capabilities = ["inspect", "compare", "register", "submit", "rebase", "revert", "cherry_pick", "apply_suggestions", "build_integration"],
  },
  policy = {
    id = "review-policy-v1",
    eligible_reviewers = ["reviewer"],
    submitters = ["operator"],
    minimum_approvals = 1,
    require_ci = true,
    ownership = {
      teams = { maintainers = ["reviewer"] },
      rules = [{ id = "source", paths = ["src/**"], owners = [], teams = ["maintainers"], target_refs = [] }],
    },
  },
  read_principals = ["author", "reviewer", "operator"],
  write_principals = ["author", "reviewer", "operator"],
  signature_name = "Kiln review service",
  signature_email = "kiln@localhost",
  intake = { default_target_ref = "refs/heads/main", mappings = [] },
  delivery = null,
  queue = { schema = "kiln.review-queue.v1", max_entries = 100, max_candidate_builds = 16 },
}
