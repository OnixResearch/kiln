let positive = import "web-service-positive.ncl" in
(import "../web-service.ncl") (positive & { bounds | force = positive.bounds & { max_requests | force = 1 } })
