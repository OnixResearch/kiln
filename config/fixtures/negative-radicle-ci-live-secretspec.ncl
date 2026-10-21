(import "../radicle-ci-live.ncl") (
  (import "./radicle-ci-live-inputs.ncl")
  & { tools.secretspec | force = "/nix/store/00000000000000000000000000000000-secretspec-0.19.0/bin/secretspec" }
)
