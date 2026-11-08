let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { fleet | force = positive.fleet & { minimum = 4, maximum = 2 } })
