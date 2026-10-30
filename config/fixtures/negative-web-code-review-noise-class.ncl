# Noise patterns admit no character classes or partial `**` segments.
(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    review = { noise = [{ id = "class", pattern = "vendor/[ab]**", reason = "Character class" }] },
  }
)
