let input = import "web-developer-inputs.ncl" in
(import "../web-developer.ncl") (input & { poll_interval_ms | force = 1000, watch_timeout_ms | force = 500 })
