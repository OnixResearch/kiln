let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
let vm = base.runners.microvm in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      base
      & {
        runners | force =
          base.runners
          & {
            microvm | force =
              vm
              & {
                untrusted_capable = true,
                trust_domain = "shared",
                host_identity = "host-1",
              },
          },
        workload | force = [
          { organization = "acme", project = "app", runner = "microvm" },
        ],
      }
  }
)
