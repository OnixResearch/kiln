(import "../patchgate-review.ncl") (
  (import "./patchgate-review-inputs.ncl")
  & {
    limits.timeout_ms | force = 0,
  }
)
