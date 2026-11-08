let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { network | force = positive.network & { bind_address | force = "127.0.0.1:65536" } })
