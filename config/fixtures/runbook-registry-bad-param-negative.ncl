let base = import "runbook-registry-inputs.ncl" in
(import "../runbook-registry.ncl") (
  base
  & {
    runbooks | force = [
      (std.array.at 0 base.runbooks)
      & {
        params | force = [
          {
            name = "bad_env",
            type = { kind = "bool" },
            required = false,
            default = true,
            bindings = [
              { node = "audit", shell_env = "NOT_LATTICE_PARAM" },
            ],
          },
        ],
      },
    ],
    grants | force = [],
  }
)
