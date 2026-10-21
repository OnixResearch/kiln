(import "../radicle-ci-live-self-ci.ncl") (
  (import "./radicle-ci-live-self-ci-inputs.ncl")
  & {
    self_ci.base_commit | force = "not-a-commit",
  }
)
