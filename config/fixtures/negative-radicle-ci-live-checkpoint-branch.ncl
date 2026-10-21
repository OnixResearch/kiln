(import "../radicle-ci-live.ncl") (
  (import "./radicle-ci-live-inputs.ncl")
  & { status_checkpoint_branch | force = "main" }
)
