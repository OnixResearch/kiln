let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { workload | force = [execution.workload [0] & { runner | force = "missing" }] } })
