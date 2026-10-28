let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { heartbeat | force = input.heartbeat & { slots | force = 0 } })
