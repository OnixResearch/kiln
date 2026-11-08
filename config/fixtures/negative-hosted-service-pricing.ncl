let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { pricing | force = [execution.pricing [0], execution.pricing [0]] } })
