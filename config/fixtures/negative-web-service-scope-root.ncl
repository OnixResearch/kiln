let positive = import "web-service-positive.ncl" in
let project = positive.projects [0] in
(import "../web-service.ncl") (positive & { projects | force = [project & { scope | force = { root = "src/*" } }] })
