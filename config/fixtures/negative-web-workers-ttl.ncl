let input = import "web-workers-inputs.ncl" in
(import "../web-workers.ncl") (input & { maximum_ttl_ms | force = 0 })
