# Warm pools are bounded to 16 slots.
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
                warm_pool | force = { directory = "/private/microvm-pool", slots = 17 },
              },
          },
      },
  }
)
