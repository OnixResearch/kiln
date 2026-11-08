let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { snapshot_limits | force = { max_file_bytes = 33554432, max_total_bytes = 16777216 } })
