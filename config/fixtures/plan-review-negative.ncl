# Rejected: the plan's author is also its reviewer, so the review would
# approve its own plan.
let contracts = import "../plan-review-contracts.ncl" in
{
  schema = "kiln.plan-review.cases.v1",
  work_identity = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  reviewer = "agent",
  cases = [
    {
      id = "self-review",
      provenance = 'synthetic,
      author = "agent",
      revisions = [[{ id = "work", text = "Do the work." }]],
      annotations = [],
      resolutions = [],
      decisions = [{ revision = 1, verdict = 'accepted }],
    },
  ],
}
  | contracts.Cases
  | contracts.Consistent
