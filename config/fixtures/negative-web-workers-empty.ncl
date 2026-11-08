let input = import "web-workers-inputs.ncl" in
(import "../web-workers.ncl") (input & { workers | force = [] })
