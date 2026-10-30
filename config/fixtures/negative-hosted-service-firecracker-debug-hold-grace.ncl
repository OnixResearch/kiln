# Debug hold grace is bounded to 900 seconds.
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
                debug_hold | force = { enabled = true, grace_seconds = 901, max_concurrent = 1 },
              },
          },
      },
  }
)
