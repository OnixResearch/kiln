let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = base & { runners | force = base.runners & { local | force = base.runners.local & { runtime_roots | force = [] } } } })
