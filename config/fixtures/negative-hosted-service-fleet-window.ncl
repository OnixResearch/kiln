let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { fleet | force = positive.fleet & { minimum | force = 2, maximum | force = 1 } })
