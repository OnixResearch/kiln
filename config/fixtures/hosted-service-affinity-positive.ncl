let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      base
      & {
        taskset | force = base.systemctl,
        runners | force =
          base.runners
          & {
            microvm | force = base.runners.microvm & { cpu_affinity | force = "0-3,8" },
          },
      },
  }
)
