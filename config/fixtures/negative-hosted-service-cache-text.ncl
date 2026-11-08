let positive = import "hosted-service-positive.ncl" in
let cache = positive.execution.cache in
let policy = cache.policies [0] in
(import "../hosted-service.ncl") (positive & { execution | force = positive.execution & { cache | force = cache & { policies | force = [policy & { worker | force = policy.worker & { producer | force = "" } }] } } })
