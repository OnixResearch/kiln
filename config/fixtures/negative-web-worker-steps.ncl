let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { steps | force = [] })
