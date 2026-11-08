let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { extra = 1 })
