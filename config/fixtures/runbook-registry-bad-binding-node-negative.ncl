let base = import "runbook-registry-inputs.ncl" in
(import "../runbook-registry.ncl") (
  base
  & {
    runbooks | force = [
      (std.array.at 0 base.runbooks)
      & {
        params | force = [
          {
            name = "profile",
            type = { kind = "enum", values = ["fast", "full"] },
            required = false,
            default = "fast",
            bindings = [
              { node = "missing-node", shell_env = "LATTICE_PARAM_PROFILE" },
            ],
          },
        ],
      },
    ],
    grants | force = [],
  }
)
