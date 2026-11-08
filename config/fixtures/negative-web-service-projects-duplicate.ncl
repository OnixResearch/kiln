let positive = import "web-service-positive.ncl" in
(import "../web-service.ncl") (positive & { projects | force = [positive.projects [0], positive.projects [0]] })
