let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = base & { runners | force = base.runners & { microvm | force = base.runners.microvm & { version | force = "   " } } } })
