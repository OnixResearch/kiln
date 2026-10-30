let input = import "web-code-review-inputs.ncl" in
(import "../web-code-review.ncl") (
  input
  & {
    queue | force = input.queue & { mode = "speculative", window = { floor = 1, ceiling = 3, increase = 1, decrease = 1 } },
  }
)
