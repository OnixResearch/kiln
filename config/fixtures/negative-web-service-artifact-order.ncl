let positive = import "web-service-positive.ncl" in
(import "../web-service.ncl") (positive & { bounds | force = positive.bounds & { max_artifact_bytes | force = 8388608 } })
