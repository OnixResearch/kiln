let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { execution | force = positive.execution & { extra = 1 } })
