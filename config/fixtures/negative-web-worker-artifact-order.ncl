let input = import "web-worker-inputs.ncl" in
(import "../web-worker.ncl") (input & { maximum_artifact_bytes | force = 33554432, maximum_total_artifact_bytes | force = 16777216 })
