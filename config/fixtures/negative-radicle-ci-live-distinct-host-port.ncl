(import "../radicle-ci-live-distinct-host.ncl") (
  (import "./radicle-ci-live-distinct-host-inputs.ncl")
  & {
    remote.port | force = 0,
  }
)
