(import "../radicle-ci-live.ncl") (
  (import "./radicle-ci-live-inputs.ncl")
  & { source_branch | force = "main" }
)
