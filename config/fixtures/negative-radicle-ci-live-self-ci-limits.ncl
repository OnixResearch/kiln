(import "../radicle-ci-live-self-ci.ncl") (
  (import "./radicle-ci-live-self-ci-inputs.ncl")
  & {
    self_ci.source_limits.max_entries | force = 512,
  }
)
