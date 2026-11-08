let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = base & { maximum_timeout_ms | force = 5000 } })
