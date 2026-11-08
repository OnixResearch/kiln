let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { network | force = positive.network & { relay_urls | force = ["https://[::ffff:999.0.0.1]:80"] } })
