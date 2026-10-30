# A Firecracker runner may be declared untrusted-capable.
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
            microvm | force = positive.execution.runners.microvm & { untrusted_capable = true },
          },
      },
  }
)
