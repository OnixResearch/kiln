let base = import "runbook-registry-inputs.ncl" in
(import "../runbook-registry.ncl") (
  base
  & {
    runbooks | force = [
      std.array.at 0 base.runbooks,
      std.array.at 0 base.runbooks,
    ],
    grants | force = [],
  }
)
