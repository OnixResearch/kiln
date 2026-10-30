# Noise patterns use the ownership glob grammar: no `..` segments.
(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    review = { noise = [{ id = "escape", pattern = "vendor/../src/**", reason = "Escapes its root" }] },
  }
)
