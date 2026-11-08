let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { fleet | force = positive.fleet & { organizations | force = [] } })
