let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { fleet | force = positive.fleet & { pricing | force = positive.fleet.pricing & { extra = 1 } } })
