(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    intake | force = {
      default_target_ref = "refs/heads/main",
      mappings = [{ source_ref = "refs/heads/review/one", target_ref = "refs/heads/main", principal = "author" }],
    },
  }
)
