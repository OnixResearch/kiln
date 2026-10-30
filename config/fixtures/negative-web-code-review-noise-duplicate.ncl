# Noise rule ids are unique.
(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    review = {
      noise = [
        { id = "vendor", pattern = "vendor/**", reason = "Vendored" },
        { id = "vendor", pattern = "third_party/**", reason = "Also vendored" },
      ],
    },
  }
)
