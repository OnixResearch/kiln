let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
let vm = base.runners.microvm in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      base
      & {
        taskset | force = null,
        runners | force =
          base.runners
          & {
            microvm | force =
              vm
              & {
                trust_domain | force = null,
                host_identity | force = null,
                cpu_affinity | force = null,
                debug_hold | force = null,
              },
          },
      }
  }
)
