let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { trusted_projects | force = [execution.trusted_projects [0] & { extra = 1 }] } })
