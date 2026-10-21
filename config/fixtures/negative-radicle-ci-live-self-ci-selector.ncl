(import "../radicle-ci-live-self-ci.ncl") (
  (import "./radicle-ci-live-self-ci-inputs.ncl")
  & {
    self_ci.root_selector | force = "jobs",
  }
)
