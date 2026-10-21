(import "../radicle-ci-live-distinct-host.ncl") (
  (import "./radicle-ci-live-distinct-host-inputs.ncl")
  & {
    remote.host | force = "aspen1; touch /tmp/escaped",
  }
)
