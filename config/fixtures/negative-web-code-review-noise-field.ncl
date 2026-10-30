# Noise rules are closed records: no authority-like extra fields.
(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    review = { noise = [{ id = "vendor", pattern = "vendor/**", reason = "Vendored", skip_ownership = true }] },
  }
)
