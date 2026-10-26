let input = import "web-service-inputs.ncl" in
(import "../web-service.ncl") (input & { bounds | force = input.bounds & { max_artifact_bytes | force = 0 } })
