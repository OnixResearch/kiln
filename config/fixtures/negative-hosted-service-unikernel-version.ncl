let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
let executable = execution.systemd_run in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { runners | force = execution.runners & { unikernel | force = { kind = "unikernel-test", firecracker = executable, kernel = executable, project = "unikraft", version = "", vcpus = 1, memory_mib = 32 } } } })
