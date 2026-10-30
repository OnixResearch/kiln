# A debug hold has no per-run override and no unknown fields.
let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      positive.execution
      & {
        runners | force =
          positive.execution.runners
          & {
            microvm | force =
              positive.execution.runners.microvm
              & {
                debug_hold | force = { enabled = true, grace_seconds = 60, max_concurrent = 1, allow_secrets = true },
              },
          },
      },
  }
)
