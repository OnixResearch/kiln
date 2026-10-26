(import "../patchgate-review.ncl") (
  (import "./patchgate-review-inputs.ncl")
  & {
    native.main.state | force = "ambiguous-prefix",
  }
)
