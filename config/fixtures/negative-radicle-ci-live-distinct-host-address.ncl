(import "../radicle-ci-live-distinct-host.ncl") (
  (import "./radicle-ci-live-distinct-host-inputs.ncl")
  & {
    remote.address | force = "192.168.1.239",
  }
)
