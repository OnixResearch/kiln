let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
let long = "/private/" ++ std.string.replicate "x" 90 in
(import "../hosted-service.ncl") (positive & { execution | force = base & { runners | force = base.runners & { microvm | force = base.runners.microvm & { warm_pool | force = { directory = long, slots = 1 } } } } })
