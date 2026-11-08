let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { network | force = positive.network & { relay_urls | force = ["https://[12345::]:80"] } })
