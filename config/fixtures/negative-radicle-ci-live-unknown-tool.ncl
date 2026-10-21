(import "../radicle-ci-live.ncl") (
  (import "./radicle-ci-live-inputs.ncl")
  & { tools.ssh = "/nix/store/00000000000000000000000000000000-ssh-fixture/bin/ssh" }
)
