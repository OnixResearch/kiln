(import "../radicle-ci-live-distinct-host.ncl") (
  (import "./radicle-ci-live-distinct-host-inputs.ncl")
  & {
    remote.root | force = "/home/brittonr/canary",
  }
)
