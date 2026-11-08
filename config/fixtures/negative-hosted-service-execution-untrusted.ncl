let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { runners | force = execution.runners & { microvm | force = execution.runners.microvm & { untrusted_capable | force = true } } } })
