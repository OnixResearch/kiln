let execution = (import "hosted-service-positive.ncl").execution in
let workload = execution.workload [0] in
(import "../hosted-execution.ncl") (execution & { runners | force = { local = execution.runners.local }, workload | force = [workload & { fleet | force = null }] })
