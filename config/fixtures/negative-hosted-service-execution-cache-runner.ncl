let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
let cache = execution.cache in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { cache | force = cache & { policies | force = [cache.policies [0] & { runner | force = "missing" }] } } })
