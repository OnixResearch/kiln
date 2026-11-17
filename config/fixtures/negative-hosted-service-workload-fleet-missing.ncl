let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
let workload = execution.workload [0] in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { workload | force = [workload & { fleet | force = null }] } })
