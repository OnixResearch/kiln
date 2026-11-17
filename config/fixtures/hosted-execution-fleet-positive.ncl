let execution = (import "hosted-service-positive.ncl").execution in
(import "../hosted-execution.ncl") (execution & { runners | force = { local = execution.runners.local } })
