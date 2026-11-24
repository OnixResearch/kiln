let base = import "runbook-registry-inputs.ncl" in
(import "../runbook-registry.ncl") (
  base
  & {
    grants | force = [
      {
        principal = "local:*",
        runbooks = ["flake-update"],
      },
    ],
  }
)
