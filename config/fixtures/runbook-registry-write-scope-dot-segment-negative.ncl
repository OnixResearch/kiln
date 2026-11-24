let base = import "runbook-registry-inputs.ncl" in
(import "../runbook-registry.ncl") (
  base
  & {
    runbooks | force = [
      (std.array.at 1 base.runbooks)
      & {
        fanout | force = {
          enabled = true,
          default_max_concurrency = 1,
          max_concurrency_limit = 4,
          default_budget_cap = 8,
          max_budget_cap = 32,
          max_targets = 16,
          write_scopes = ["./crates"],
        },
      },
    ],
    grants | force = [],
  }
)
