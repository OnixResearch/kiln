let positive = import "web-service-positive.ncl" in
(import "../web-service.ncl") (positive & { projects | force = [] })
