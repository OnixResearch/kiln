let positive = import "web-service-positive.ncl" in
(import "../web-service.ncl") (positive & { worker_token_file | force = positive.operator_token_file })
