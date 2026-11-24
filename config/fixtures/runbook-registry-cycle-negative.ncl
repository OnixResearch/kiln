let base = import "runbook-registry-inputs.ncl" in
let flake_update = std.array.at 1 base.runbooks in
(import "../runbook-registry.ncl") (
  base
  & {
    runbooks | force = [
      flake_update
      & {
        lattice = {
          edges | force = [
            { source = "update", target = "verify" },
            { source = "verify", target = "update" },
          ],
        },
      },
    ],
  }
)
