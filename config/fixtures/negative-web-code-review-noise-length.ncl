# Noise patterns are bounded to 256 characters.
(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    review = { noise = [{ id = "long", pattern = std.string.join "/" (std.array.replicate 129 "a"), reason = "Unbounded" }] },
  }
)
