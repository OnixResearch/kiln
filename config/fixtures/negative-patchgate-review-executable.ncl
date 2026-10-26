(import "../patchgate-review.ncl") (
  (import "./patchgate-review-inputs.ncl")
  & {
    executable_identity | force = "not-a-content-identity",
  }
)
