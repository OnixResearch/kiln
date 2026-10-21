# Synthetic admission values; the runner requires the real pinned cohort.
(import "./radicle-ci-live-inputs.ncl")
& {
  remote = {
    host = "aspen1",
    root = "/var/tmp/kiln-radicle-live-distinct-host-fixture",
    address = "100.100.103.95",
    port = 52000,
    readiness_seconds = 30,
  },
}
