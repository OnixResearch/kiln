(import "../radicle-ci-live.ncl") (
  (import "./radicle-ci-live-inputs.ncl")
  & { tools.radicle | force = "/usr/bin/rad" }
)
