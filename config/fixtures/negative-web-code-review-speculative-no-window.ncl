let input = import "web-code-review-inputs.ncl" in
(import "../web-code-review.ncl") (
  input
  & {
    provider | force = input.provider & { capabilities | force = input.provider.capabilities @ ["speculative_integration"] },
    queue | force = input.queue & { mode = "speculative" },
  }
)
