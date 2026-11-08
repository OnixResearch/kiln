let input = import "web-workers-inputs.ncl" in
(import "../web-workers.ncl") (input & { workers | force = [input.workers [0], input.workers [0]] })
