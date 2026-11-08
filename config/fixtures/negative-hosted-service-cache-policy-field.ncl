let positive = import "hosted-service-positive.ncl" in
let cache = positive.execution.cache in
(import "../hosted-service.ncl") (positive & { execution | force = positive.execution & { cache | force = cache & { policies | force = [cache.policies [0] & { extra = 1 }] } } })
