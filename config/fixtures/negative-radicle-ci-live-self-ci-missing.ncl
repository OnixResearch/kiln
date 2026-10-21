(import "../radicle-ci-live-self-ci.ncl") (
  (import "./radicle-ci-live-distinct-status-inputs.ncl")
  & {
    self_ci | force = {},
  }
)
