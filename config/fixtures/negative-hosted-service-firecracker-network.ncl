# A Firecracker runner has no network field; unknown fields are rejected.
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
                network_interfaces = [{ iface_id = "eth0", host_dev_name = "tap0" }],
              },
          },
      },
  }
)
