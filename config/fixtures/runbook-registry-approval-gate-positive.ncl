let base = import "runbook-registry-inputs.ncl" in
(import "../runbook-registry.ncl") (
  base
  & {
    runbooks | force = [
      (std.array.at 0 base.runbooks)
      & {
        approval_gate = "none",
      },
      (std.array.at 1 base.runbooks)
      & {
        approval_gate = "kiln_publication",
      },
    ],
  }
)
